use rapier3d::prelude::{ColliderHandle, ContactPair, NarrowPhase, RigidBody};
use ultraviolet::{Rotor3, Vec3};

use crate::event_loop::Inputs;

fn normalize_if_not_zero(v: Vec3) -> Vec3 {
    if v != Vec3::zero() {
        v.normalized()
    } else {
        Vec3::zero()
    }
}

const FLOOR_COLLISION_HEIGHT: f32 = -0.5;
const JUMP_CUTOFF: f32 = 0.5;
const JUMP_VELOCITY: f32 = 4.0;
const MOVEMENT_SPEED: f32 = 40.0;
const AIR_STRAFE_SPEED: f32 = 10.0;
const TOP_SPEED: f32 = 7.0;
const COYOTE_TIME: f32 = 0.2;
fn is_on_floor(narrow_phase: &NarrowPhase, player_collider_handle: ColliderHandle) -> bool {
    let is_colliding_with_floor = |contact_pair: &ContactPair| {
        let opposite_actually = if contact_pair.collider1 == player_collider_handle {
            false
        } else if contact_pair.collider2 == player_collider_handle {
            true
        } else {
            panic!("expected collision to involve player");
        };

        if contact_pair.has_any_active_contact {
            for manifold in &contact_pair.manifolds {
                for point in &manifold.points {
                    if point.dist <= 0.0
                        && (if opposite_actually {
                            point.local_p2.y
                        } else {
                            point.local_p1.y
                        } < FLOOR_COLLISION_HEIGHT)
                    {
                        return true;
                    }
                }
            }
        }

        false
    };

    narrow_phase
        .contact_pairs_with(player_collider_handle)
        .any(is_colliding_with_floor)
}

pub fn get_movement_impulse(
    narrow_phase: &NarrowPhase,
    player_collider_handle: ColliderHandle,
    inputs: &Inputs,
    player_info: &RigidBody,
    camera_rotation: Rotor3,
    dt: f32,
    time_since_left_ground: &mut f32,
) -> Vec3 {
    let movement = if inputs.forward {
        Vec3::unit_z()
    } else if inputs.backward {
        -Vec3::unit_z()
    } else {
        Vec3::zero()
    } + if inputs.left {
        Vec3::unit_x()
    } else if inputs.right {
        -Vec3::unit_x()
    } else {
        Vec3::zero()
    };

    let is_on_floor = is_on_floor(narrow_phase, player_collider_handle);

    let is_jumping = inputs.up && *time_since_left_ground <= COYOTE_TIME;

    if !is_on_floor {
        *time_since_left_ground += dt;
    } else {
        *time_since_left_ground = 0.0;
    }

    let movement_direction =
        normalize_if_not_zero(movement.rotated_by(camera_rotation) * Vec3::new(1.0, 0.0, 1.0));

    let horizontal_velocity =
        Vec3::from(player_info.linvel().as_slice().first_chunk::<3>().unwrap())
            * Vec3::new(1.0, 0.0, 1.0);

    let correction = if horizontal_velocity != Vec3::zero() {
        let movement_direction = if movement_direction == Vec3::zero() {
            horizontal_velocity / 1.5
        } else {
            movement_direction
        };

        let speed_in_direction = (horizontal_velocity
            * (horizontal_velocity.dot(movement_direction)
                / horizontal_velocity.dot(horizontal_velocity)))
        .mag();

        horizontal_velocity * (1.0 - ((TOP_SPEED - speed_in_direction) / TOP_SPEED))
    } else {
        Vec3::zero()
    };

    player_info.mass()
        * (if is_on_floor {
            MOVEMENT_SPEED
        } else {
            AIR_STRAFE_SPEED
        } * (movement_direction - correction)
            * dt
            + if is_jumping && (player_info.linvel().y < JUMP_CUTOFF) {
                *time_since_left_ground = f32::MAX;
                (JUMP_VELOCITY - player_info.linvel().y) * Vec3::unit_y()
            } else {
                Vec3::zero()
            })
}
