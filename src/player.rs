use rapier3d::{
    math::{Point, Vector},
    na::vector,
    parry::query::DefaultQueryDispatcher,
    prelude::{
        ColliderBuilder, ColliderHandle, ContactPair, NarrowPhase, QueryFilter, Ray, Real,
        RigidBodyBuilder, RigidBodyHandle,
    },
};
use ultraviolet::{Rotor3, Vec3};

use crate::{
    event_loop::SharedState,
    physics::{Physics, from_nalgebra},
};

const HALF_HEIGHT: f32 = 0.9;
const RADIUS: f32 = 0.2;

const FLOOR_COLLISION_HEIGHT: f32 = -RADIUS;
const JUMP_CUTOFF: f32 = 2.0;
const JUMP_VELOCITY: f32 = 5.2;
const MOVEMENT_SPEED: f32 = 40.0;
const AIR_STRAFE_SPEED: f32 = 10.0;
const TOP_SPEED: f32 = 5.0;
const COYOTE_TIME: f32 = 0.1;
const FLOOR_DRAG: f32 = 1.0 / 2.0;
const AIR_DRAG: f32 = 1.0 / 50.0;
const STATIC_FRICTION_CUTOFF: f32 = 3.0;
const MAX_STATIC_FRICTION: f32 = 4.0;
const MAX_SLOPE: f32 = 0.2;

#[derive(Debug)]
pub struct Player {
    pub position: Vec3,
    pub previous_position: Vec3,
    pub collider_handle: ColliderHandle,
    pub rigid_body_handle: RigidBodyHandle,
    previous_floor_contact: Option<FloorContact>,
    time_since_left_ground: f32,
    jump_buffer: bool,
    was_jumping: bool,
}

impl Player {
    pub fn new(physics: &mut Physics) -> Self {
        let collider = ColliderBuilder::capsule_y(HALF_HEIGHT, RADIUS)
            .restitution(0.0)
            .friction(0.0)
            .friction_combine_rule(rapier3d::prelude::CoefficientCombineRule::Multiply)
            .density(4.0);

        let rigid_body_handle = physics.rigid_body_set.insert(
            RigidBodyBuilder::dynamic()
                .translation(vector![0.0, HALF_HEIGHT + RADIUS, 0.0])
                .lock_rotations(),
        );

        let collider_handle = physics.collider_set.insert_with_parent(
            collider,
            rigid_body_handle,
            &mut physics.rigid_body_set,
        );

        let position =
            from_nalgebra(physics.rigid_body_set[rigid_body_handle].position()).translation;

        Self {
            position,
            previous_position: position,
            collider_handle,
            rigid_body_handle,
            previous_floor_contact: None,
            time_since_left_ground: f32::MAX,
            jump_buffer: false,
            was_jumping: false,
        }
    }

    pub fn update(
        &mut self,
        physics: &mut Physics,
        input: &SharedState,
        camera_rotation: Rotor3,
        dt: f32,
    ) {
        self.jump_buffer = match (input.up(), input.previous.up()) {
            (true, false) => true,
            (false, true) => false,
            _ => self.jump_buffer,
        };

        let floor_contact = floor_contact(&physics.narrow_phase, self.collider_handle);

        let rigid_body = &physics.rigid_body_set[self.rigid_body_handle];

        let is_jumping = input.up()
            && self.time_since_left_ground <= COYOTE_TIME
            && rigid_body.linvel().y < JUMP_CUTOFF
            && self.jump_buffer;

        let on_floor = floor_contact.is_some();

        let cast_ray = |previous_floor_contact: &FloorContact| {
            let projection_distance = 0.05;

            physics
                .broad_phase
                .as_query_pipeline(
                    &DefaultQueryDispatcher {},
                    &physics.rigid_body_set,
                    &physics.collider_set,
                    QueryFilter::new().exclude_rigid_body(self.rigid_body_handle),
                )
                .cast_ray_and_get_normal(
                    &Ray::new(
                        previous_floor_contact.point + rigid_body.position().translation.vector
                            - projection_distance * previous_floor_contact.normal,
                        previous_floor_contact.normal,
                    ),
                    2.0 * projection_distance,
                    true,
                )
                .and_then(|floor_raycast| {
                    let normal = &-floor_raycast.1.normal;

                    ((previous_floor_contact.normal.dot(normal) >= 0.75) && (is_floor(normal)))
                        .then(|| {
                            let linvel = rigid_body.linvel();
                            let velocity = -(linvel.dot(normal) * normal);

                            let summed = velocity + linvel;
                            let normalised =
                                (summed * (linvel.magnitude() / summed.magnitude())) - linvel;

                            (normalised * 1.0 * rigid_body.mass(), *normal)
                        })
                })
        };

        let floor_correction = (!on_floor && !self.was_jumping)
            .then(|| self.previous_floor_contact.as_ref().and_then(cast_ray))
            .flatten();

        let on_floor = if let Some((_, normal)) = floor_correction {
            self.previous_floor_contact = Some(FloorContact {
                point: self.previous_floor_contact.unwrap().point,
                normal,
            });

            true
        } else {
            self.previous_floor_contact = floor_contact;

            on_floor
        };

        self.was_jumping = is_jumping;

        let movement = if input.forward() {
            Vec3::unit_z()
        } else if input.backward() {
            -Vec3::unit_z()
        } else {
            Vec3::zero()
        } + if input.left() {
            Vec3::unit_x()
        } else if input.right() {
            -Vec3::unit_x()
        } else {
            Vec3::zero()
        };

        if on_floor {
            self.time_since_left_ground = 0.0;
        } else {
            self.time_since_left_ground += dt;
        }

        let movement_direction =
            normalize_if_not_zero(movement.rotated_by(camera_rotation) * Vec3::new(1.0, 0.0, 1.0));

        let horizontal_velocity =
            Vec3::from(rigid_body.linvel().as_slice().first_chunk::<3>().unwrap())
                * Vec3::new(1.0, 0.0, 1.0);

        let correction = if horizontal_velocity == Vec3::zero() {
            Vec3::zero()
        } else {
            let movement_direction = if movement_direction == Vec3::zero() {
                horizontal_velocity * if on_floor { FLOOR_DRAG } else { AIR_DRAG }
            } else {
                movement_direction
            };

            let speed_in_direction = (horizontal_velocity
                * (horizontal_velocity.dot(movement_direction)
                    / horizontal_velocity.dot(horizontal_velocity)))
            .mag();

            horizontal_velocity * (1.0 - ((TOP_SPEED - speed_in_direction) / TOP_SPEED))
        };

        let friction = if rigid_body.linvel().magnitude() < STATIC_FRICTION_CUTOFF
            && movement == Vec3::zero()
        {
            MAX_STATIC_FRICTION
                * ((STATIC_FRICTION_CUTOFF - rigid_body.linvel().magnitude())
                    / STATIC_FRICTION_CUTOFF)
        } else {
            0.0
        };

        let impulse = rapier3d::na::Vector3::from_row_slice(
            (rigid_body.mass()
                * (if on_floor {
                    MOVEMENT_SPEED
                } else {
                    AIR_STRAFE_SPEED
                } * (movement_direction - correction)
                    * dt
                    + if is_jumping {
                        self.time_since_left_ground = f32::MAX;
                        self.jump_buffer = false;
                        (JUMP_VELOCITY - rigid_body.linvel().y) * Vec3::unit_y()
                    } else {
                        Vec3::zero()
                    }))
            .as_slice(),
        ) + floor_correction
            .unwrap_or_else(|| (Vector::zeros(), Vector::default()))
            .0;

        physics.collider_set[self.collider_handle].set_friction(friction);
        physics.rigid_body_set[self.rigid_body_handle].apply_impulse(impulse, true);
    }
}

#[derive(Debug, Copy, Clone)]
struct FloorContact {
    point: Point<Real>,
    normal: Vector<Real>,
}

fn is_floor(normal: &Vector<Real>) -> bool {
    normal.dot(&vector![0.0, 1.0, 0.0]).abs() > MAX_SLOPE
}

fn floor_contact(
    narrow_phase: &NarrowPhase,
    player_collider_handle: ColliderHandle,
) -> Option<FloorContact> {
    let is_colliding_with_floor = |contact_pair: &ContactPair| {
        let opposite_actually = if contact_pair.collider1 == player_collider_handle {
            false
        } else if contact_pair.collider2 == player_collider_handle {
            true
        } else {
            panic!("expected collision to involve player");
        };

        if !contact_pair.has_any_active_contact {
            return None;
        }

        for manifold in &contact_pair.manifolds {
            if !is_floor(&manifold.data.normal) {
                return None;
            }

            for point in &manifold.points {
                if point.dist > 0.0 {
                    return None;
                }

                let point_of_interest = if opposite_actually {
                    point.local_p2
                } else {
                    point.local_p1
                };

                if point_of_interest.y < FLOOR_COLLISION_HEIGHT {
                    return Some(FloorContact {
                        point: point_of_interest,
                        normal: manifold.data.normal,
                    });
                }
            }
        }

        None
    };

    narrow_phase
        .contact_pairs_with(player_collider_handle)
        .find_map(is_colliding_with_floor)
}

fn normalize_if_not_zero(v: Vec3) -> Vec3 {
    if v == Vec3::zero() {
        Vec3::zero()
    } else {
        v.normalized()
    }
}
