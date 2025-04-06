use ultraviolet::{Bivec3, Rotor3, Vec3};

#[derive(Clone, Debug)]
pub struct RigidBody {
    pub position: Vec3,
    pub orientation: Rotor3,
    pub velocity: Vec3,
    pub angular_momentum: Vec3,
    pub inverse_moment_of_inertia: Vec3, // Moment of inertia about a principle axis
    pub mass: f32,
}

impl RigidBody {
    pub fn new(
        position: Vec3,
        orientation: Rotor3,
        velocity: Vec3,
        angular_momentum: Vec3,
        moment_of_inertia: Vec3,
        mass: f32,
    ) -> Self {
        RigidBody {
            position,
            orientation,
            velocity,
            angular_momentum,
            inverse_moment_of_inertia: Vec3::new(
                1.0 / moment_of_inertia.x,
                1.0 / moment_of_inertia.y,
                1.0 / moment_of_inertia.z,
            ),
            mass,
        }
    }

    pub fn update(&mut self, dt: f32) {
        let angular_velocity = (self.inverse_moment_of_inertia
            * self
                .angular_momentum
                .rotated_by(self.orientation.reversed()))
        .rotated_by(self.orientation);

        self.orientation = (self.orientation
            * Rotor3::from_angle_plane(
                angular_velocity.mag() * (0.5 * dt),
                Bivec3::from_normalized_axis(angular_velocity / angular_velocity.mag()),
            ))
        .normalized();
    }
}
