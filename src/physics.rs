use std::{cell::RefCell, rc::Rc};

use itertools::Itertools;
use ultraviolet::{Bivec3, Isometry3, Rotor3, Vec3};

use crate::{
    collision::{collide, TransformedPolyhedron},
    node::{GameTree, Node, Object},
};

#[derive(Clone, Debug)]
pub struct RigidBody {
    pub position: Vec3,
    pub orientation: Rotor3,
    pub velocity: Vec3,
    pub angular_momentum: Vec3,
    pub inverse_moment_of_inertia: Vec3, // Moment of inertia about a principle axis
    pub inverse_mass: f32,
}

impl RigidBody {
    pub fn new(
        position: Vec3,
        orientation: Rotor3,
        velocity: Vec3,
        angular_momentum: Vec3,
        inverse_moment_of_inertia: Vec3,
        inverse_mass: f32,
    ) -> Self {
        RigidBody {
            position,
            orientation,
            velocity,
            angular_momentum,
            inverse_moment_of_inertia,
            inverse_mass,
        }
    }

    fn apply_inverse_inertia_tensor(&self, vec: Vec3) -> Vec3 {
        (self.inverse_moment_of_inertia * vec.rotated_by(self.orientation.reversed()))
            .rotated_by(self.orientation)
    }

    fn rotate_by_vec(&mut self, vec: Vec3) {
        let mag = vec.mag();

        if mag != 0.0 {
            self.orientation = (self.orientation
                * Rotor3::from_angle_plane(mag, Bivec3::from_normalized_axis(vec / mag)))
            .normalized();
        }
    }

    pub fn update(&mut self, dt: f32) {
        const GRAVITY: f32 = -9.8;

        let acceleration = if self.inverse_mass != 0.0 {
            GRAVITY * Vec3::unit_y()
        } else {
            Vec3::zero()
        };

        self.velocity += acceleration * dt;

        self.position += self.velocity * dt;

        let angular_velocity = self.apply_inverse_inertia_tensor(self.angular_momentum);

        self.rotate_by_vec(angular_velocity * dt);
    }
}

fn update_rigidbody<F: FnMut(&mut RigidBody)>(
    node: Rc<RefCell<Node>>,
    mut f: F,
) -> (Vec3, Rotor3, Rc<RefCell<Node>>) {
    node.borrow_mut()
        .objects
        .iter_mut()
        .find_map(|object| match object {
            Object::RigidBody(ref mut rb) => {
                let ret = (rb.position, rb.orientation, node.clone());

                f(rb);

                Some(ret)
            }
            _ => None,
        })
        .unwrap()
}

/*
fn handle_collision(
    p: &mut RigidBody,
    q: &mut RigidBody,
    contact_point: Vec3,
    contact_normal: Vec3,
    contact_depth: f32,
) {
    let (p_r, q_r) = (contact_point - p.position, contact_point - q.position);

    let tensor_term = |rb: &RigidBody, r: Vec3| {
        contact_normal.dot(
            rb.apply_inverse_inertia_tensor(r.cross(contact_normal))
                .cross(r),
        )
    };

    let inv_effective_mass =
        p.inverse_mass + q.inverse_mass + tensor_term(p, p_r) + tensor_term(q, q_r);

    let lambda = contact_depth / inv_effective_mass;

    p.position -= p.inverse_mass * lambda * contact_normal;
    q.position += q.inverse_mass * lambda * contact_normal;

    let del_theta =
        |rb: &RigidBody, r: Vec3| rb.apply_inverse_inertia_tensor(r.cross(lambda * contact_normal));

    p.rotate_by_vec(-del_theta(p, p_r));
    q.rotate_by_vec(del_theta(q, q_r));
}*/

pub fn do_physics(root_node: &GameTree, dt: f32) {
    const NUM_SUBSTEPS: usize = 10;
    let sdt = dt / (NUM_SUBSTEPS as f32);
    for _ in 0..NUM_SUBSTEPS {
        let mut collisions_to_check: Vec<(TransformedPolyhedron<Vec3>, Rc<RefCell<Node>>)> = vec![];

        for (_, node) in root_node.breadth_first() {
            let mut new_transform = Isometry3::identity();

            let transform = node.borrow_mut().transform;

            for object in &mut node.borrow_mut().objects {
                match object {
                    Object::Collider(ref mut polyhedron) => {
                        collisions_to_check.push((polyhedron.transform(transform), node.clone()));
                    }
                    Object::RigidBody(ref mut rigid_body) => {
                        rigid_body.update(sdt);

                        new_transform = Isometry3::new(rigid_body.position, rigid_body.orientation);
                    }
                    _ => (),
                }
            }

            node.borrow_mut().transform = new_transform;
        }

        let _ = crate::physics::do_collisions(collisions_to_check);

        //update_velocities(&velocities_to_update, sdt);
    }
}

pub fn do_collisions(
    collisions_to_check: Vec<(TransformedPolyhedron<Vec3>, Rc<RefCell<Node>>)>,
) -> Vec<(Vec3, Rotor3, Rc<RefCell<Node>>)> {
    let mut velocities_to_update = vec![];

    for ((p, p_node), (q, q_node)) in collisions_to_check.iter().tuple_combinations() {
        if let Some((contact_point, contact_normal, contact_depth)) = collide(p, q) {
            let mut velocity_q = None;

            // extreme currying
            let velocity_p = update_rigidbody(p_node.clone(), |p_rb| {
                velocity_q = Some(update_rigidbody(q_node.clone(), |q_rb| {
                    handle_collision(p_rb, q_rb, contact_point, contact_normal, contact_depth);
                }));
            });

            velocities_to_update.push(velocity_p);
            velocities_to_update.push(velocity_q.unwrap());
        }
    }

    velocities_to_update
}

pub fn update_velocities(nodes: &Vec<(Vec3, Rotor3, Rc<RefCell<Node>>)>, dt: f32) {
    for (old_position, old_orientation, node) in nodes {
        let transform = node
            .borrow_mut()
            .objects
            .iter_mut()
            .find_map(|object| match object {
                Object::RigidBody(rb) => {
                    rb.velocity = (rb.position - *old_position) / dt;

                    let rot = rb.orientation * old_orientation.reversed();

                    let (rot_amount, bv) = rot.into_angle_plane();

                    if rb.inverse_moment_of_inertia != Vec3::zero() && rot_amount != 0.0 {
                        let angular_velocity = (rot_amount / dt) * Vec3::new(bv.xy, -bv.xz, bv.yz);

                        let invert = |i: Vec3| Vec3::new(1.0 / i.x, 1.0 / i.y, 1.0 / i.z);

                        rb.angular_momentum = (invert(rb.inverse_moment_of_inertia)
                            * angular_velocity.rotated_by(rb.orientation.reversed()))
                        .rotated_by(rb.orientation);
                    }

                    Some(Isometry3::new(rb.position, rb.orientation))
                }
                _ => None,
            })
            .unwrap();

        node.borrow_mut().transform = transform;
    }
}

fn handle_collision(
    p_rb: &mut RigidBody,
    q_rb: &mut RigidBody,
    contact_point: Vec3,
    contact_normal: Vec3,
    contact_depth: f32,
) {
    // Relative velocity at contact point
    let get_velocity_at_point = |rb: &RigidBody| {
        let angular_velocity = rb.apply_inverse_inertia_tensor(rb.angular_momentum);
        let r = contact_point - rb.position;
        (rb.velocity + angular_velocity.cross(r), r)
    };

    let (p_velocity, p_r) = get_velocity_at_point(p_rb);
    let (q_velocity, q_r) = get_velocity_at_point(q_rb);

    let relative_velocity_at_collision = q_velocity - p_velocity;

    let normal_velocity = relative_velocity_at_collision.dot(contact_normal);

    if normal_velocity > 0.0 {
        return;
    }

    const SLOP: f32 = 0.01;
    const CORRECTION_FACTOR: f32 = 0.1;

    let correction = ((contact_depth - SLOP).max(0.0) * CORRECTION_FACTOR) * contact_normal;

    let mass_fraction = p_rb.inverse_mass / (q_rb.inverse_mass + p_rb.inverse_mass);

    p_rb.position -= correction * mass_fraction;
    q_rb.position += correction * (1.0 - mass_fraction);

    const COEFFICIENT_OF_RESTITUTION: f32 = 0.5;

    let tensor_term = |rb: &RigidBody, r: Vec3| {
        contact_normal.dot(
            rb.apply_inverse_inertia_tensor(r.cross(contact_normal))
                .cross(r),
        )
    };

    let impulse_magnitude = -((1.0 + COEFFICIENT_OF_RESTITUTION) * normal_velocity
        / (p_rb.inverse_mass
            + q_rb.inverse_mass
            + tensor_term(p_rb, p_r)
            + tensor_term(q_rb, q_r)));

    let impulse_vector = impulse_magnitude * contact_normal;

    p_rb.velocity -= p_rb.inverse_mass * impulse_vector;
    q_rb.velocity += q_rb.inverse_mass * impulse_vector;

    p_rb.angular_momentum -= p_r.cross(impulse_vector);
    q_rb.angular_momentum += q_r.cross(impulse_vector);

    // friction
    //get_velocity_at_point(p_rb)
    let (p_velocity, p_r) = get_velocity_at_point(p_rb);
    let (q_velocity, q_r) = get_velocity_at_point(q_rb);

    let relative_velocity = q_velocity - p_velocity;
    let tangential_velocity =
        relative_velocity - (relative_velocity.dot(contact_normal)) * contact_normal;

    let tangential_velocity_mag = tangential_velocity.mag();
    const FRICTION_THRESHOLD: f32 = 0.0001;
    let tangential_velocity_norm = if tangential_velocity_mag > FRICTION_THRESHOLD {
        tangential_velocity / tangential_velocity_mag
    } else {
        Vec3::zero()
    };

    let tensor_term = |rb: &RigidBody, r: Vec3| {
        tangential_velocity_norm.dot(
            rb.apply_inverse_inertia_tensor(r.cross(tangential_velocity_norm))
                .cross(r),
        )
    };

    let tangential_effective_mass = 1.0
        / (p_rb.inverse_mass + q_rb.inverse_mass + tensor_term(p_rb, p_r) + tensor_term(q_rb, q_r));

    const COEFFICIENT_OF_FRICTION: f32 = 0.2;
    let tangential_relative_velocity =
        (-tangential_effective_mass * (tangential_velocity.dot(tangential_velocity_norm))).clamp(
            -COEFFICIENT_OF_FRICTION * impulse_magnitude.abs(),
            COEFFICIENT_OF_FRICTION * impulse_magnitude.abs(),
        ) * tangential_velocity_norm;

    p_rb.velocity -= p_rb.inverse_mass * tangential_relative_velocity;
    q_rb.velocity += q_rb.inverse_mass * tangential_relative_velocity;

    p_rb.angular_momentum -= p_r.cross(tangential_relative_velocity);
    q_rb.angular_momentum += q_r.cross(tangential_relative_velocity);
}
