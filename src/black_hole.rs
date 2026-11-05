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
    position: Vec3::new(420.0, 95.0, -800.0),
    horizon: 44.0,
    disk_radius: 195.0,
    influence: 320.0,
};

impl BlackHole {
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
        let segment = to - from;
        let t = if segment.norm_squared() > 0.0 {
            ((self.position - from).dot(&segment) / segment.norm_squared()).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (from + segment * t - self.position).norm_squared() <= self.horizon.powi(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
