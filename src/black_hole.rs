use nalgebra_glm::Vec3;

/// Shared geometry and gameplay field, in the same units as ships and planets.
#[derive(Clone, Copy)]
pub struct BlackHole {
    pub position: Vec3,
    pub horizon: f32,
    pub disk_radius: f32,
    pub influence: f32,
}

pub const BLACK_HOLE: BlackHole = BlackHole {
    position: Vec3::new(4400.0, 480.0, -4600.0),
    horizon: 44.0,
    disk_radius: 195.0,
    influence: 320.0,
};

impl BlackHole {
    /// Free debris feels gravity at every distance, independently of its size or mass.
    /// Softening and a finite acceleration keep the arcade simulation well behaved.
    pub fn free_fall_acceleration(&self, position: Vec3) -> Vec3 {
        let offset = self.position - position;
        let squared = offset.norm_squared();
        if !squared.is_finite() || squared < 1e-12 {
            return Vec3::zeros();
        }
        let magnitude = (720_000.0 / (squared + self.horizon.powi(2))).min(260.0);
        offset / squared.sqrt() * magnitude
    }

    pub fn strength(&self, position: Vec3) -> f32 {
        let t = (1.0 - (position - self.position).norm() / self.influence).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// Bounded arcade attraction, with a smooth cutoff outside the field.
    pub fn acceleration(&self, position: Vec3) -> Vec3 {
        let offset = self.position - position;
        let distance = offset.norm();
        if distance <= 0.001 || distance >= self.influence {
            return Vec3::zeros();
        }
        let acceleration =
            (720_000.0 / distance.max(self.horizon).powi(2) * self.strength(position)).min(260.0);
        offset / distance * acceleration
    }

    /// Swept collision prevents fast ships from skipping across the horizon.
    pub fn captures(&self, from: Vec3, to: Vec3) -> bool {
        self.captures_sphere(from, to, 0.0)
    }

    /// Finite rocks disappear when their leading surface crosses the event horizon.
    pub fn captures_sphere(&self, from: Vec3, to: Vec3, radius: f32) -> bool {
        let segment = to - from;
        let t = if segment.norm_squared() > 0.0 {
            ((self.position - from).dot(&segment) / segment.norm_squared()).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (from + segment * t - self.position).norm_squared()
            <= (self.horizon + radius.max(0.0)).powi(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_debris_feels_an_inward_field_beyond_the_navigation_warning_zone() {
        let hole = BLACK_HOLE;
        for distance in [hole.influence * 2.0, 5000.0, 20000.0] {
            let p = hole.position + Vec3::x() * distance;
            let force = hole.free_fall_acceleration(p);
            assert!(force.x < 0.0 && force.norm() > 0.0);
            assert!(force.y == 0.0 && force.z == 0.0);
            assert_eq!(
                hole.acceleration(p),
                Vec3::zeros(),
                "Pilot stabilization remains available far from the hole"
            );
        }
        let near = hole
            .free_fall_acceleration(hole.position + Vec3::x() * 5000.0)
            .norm();
        let far = hole
            .free_fall_acceleration(hole.position + Vec3::x() * 10000.0)
            .norm();
        assert!((near / far - 4.0).abs() < 0.01);
        assert_eq!(hole.free_fall_acceleration(hole.position), Vec3::zeros());
        let a = hole.position + Vec3::y() * (hole.horizon + 3.0);
        assert!(!hole.captures(a, a));
        assert!(hole.captures_sphere(a, a, 4.0));
    }

    #[test]
    fn attraction_has_a_smooth_boundary_and_strengthens_toward_the_center() {
        let p = |r| BLACK_HOLE.position + Vec3::new(r, 0.0, 0.0);
        assert_eq!(BLACK_HOLE.acceleration(Vec3::zeros()), Vec3::zeros());
        assert_eq!(BLACK_HOLE.acceleration(p(320.0)), Vec3::zeros());
        assert!(BLACK_HOLE.acceleration(p(319.99)).norm() < 0.001);
        let weak = BLACK_HOLE.acceleration(p(250.0));
        let strong = BLACK_HOLE.acceleration(p(100.0));
        assert!(weak.x < 0.0 && strong.norm() > weak.norm() * 10.0);
        assert!(
            BLACK_HOLE
                .acceleration(BLACK_HOLE.position)
                .iter()
                .all(|x| x.is_finite())
        );
    }

    #[test]
    fn crossing_the_horizon_is_detected_even_when_endpoints_are_outside() {
        let p = BLACK_HOLE.position;
        assert!(BLACK_HOLE.captures(p + Vec3::x() * 100.0, p - Vec3::x() * 100.0));
        assert!(!BLACK_HOLE.captures(p + Vec3::y() * 100.0, p + Vec3::x() * 100.0));
    }
}
