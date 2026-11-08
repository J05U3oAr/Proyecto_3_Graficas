use crate::{collision::PlanetCollider, color::Color, model::Model, renderer::WorldTransform};
use nalgebra_glm::Vec3;
use std::f32::consts::PI;

pub struct Occluder {
    pub center: Vec3,
    pub radius: f32,
}

pub struct SolarLight {
    pub position: Vec3,
    pub radius: f32,
    pub occluders: Vec<Occluder>,
}

pub struct ShipLights {
    pub accent: Color,
    pub exhausts: Vec<Vec3>,
}

impl ShipLights {
    pub fn shade(
        &self,
        sunlight: Color,
        base: Color,
        point: Vec3,
        normal: Vec3,
        view: Vec3,
        power: f32,
    ) -> Color {
        let normal = if normal.dot(&view) < 0.0 {
            -normal
        } else {
            normal
        };
        // La iluminacion auxiliar pertenece a la nave y permanece encendida durante los eclipses.
        let key = normal.dot(&Vec3::new(0.4, 0.9, -0.8).normalize()).max(0.0);
        let fill = normal
            .dot(&Vec3::new(-0.8, 0.25, 0.55).normalize())
            .max(0.0);
        let rim = (1.0 - normal.dot(&view).max(0.0)).powi(2);
        let auxiliary = 0.38 + key * 0.65 + fill * 0.35 + rim * 0.35;
        let mut engine = 0.0;
        for nozzle in &self.exhausts {
            let delta = nozzle + Vec3::new(0.0, 0.04, -0.14) - point;
            let distance = delta.norm_squared().max(0.0001);
            let diffuse = normal.dot(&(delta / distance.sqrt())).max(0.0);
            engine += power * (0.07 / (0.04 + distance)) * (0.15 + diffuse);
        }
        let channel = |sun: u8, material: u8, accent: u8, warmth: f32| {
            let tint = accent as f32 / 255.0;
            (sun as f32
                + material as f32 * auxiliary * (0.85 + tint * 0.15)
                + accent as f32 * (0.045 + rim * 0.065)
                + (material as f32 + 35.0) * engine * warmth)
                .clamp(0.0, 255.0)
                .round() as u8
        };
        Color::new(
            channel(sunlight.r, base.r, self.accent.r, 1.0),
            channel(sunlight.g, base.g, self.accent.g, 0.38),
            channel(sunlight.b, base.b, self.accent.b, 0.09),
        )
    }
}

fn disc_overlap(a: f32, b: f32, distance: f32) -> f32 {
    if distance >= a + b {
        return 0.0;
    }
    if distance <= (a - b).abs() {
        return PI * a.min(b).powi(2);
    }
    let angle_a = ((distance * distance + a * a - b * b) / (2.0 * distance * a))
        .clamp(-1.0, 1.0)
        .acos();
    let angle_b = ((distance * distance + b * b - a * a) / (2.0 * distance * b))
        .clamp(-1.0, 1.0)
        .acos();
    let area = ((-distance + a + b) * (distance + a - b) * (distance - a + b) * (distance + a + b))
        .max(0.0)
        .sqrt();
    a * a * angle_a + b * b * angle_b - area * 0.5
}

impl SolarLight {
    pub fn visibility(&self, point: Vec3) -> f32 {
        let relative = self.position - point;
        let distance = relative.norm().max(0.001);
        let direction = relative / distance;
        let angular_sun = (self.radius / distance).clamp(0.00001, 0.9999).asin();
        let mut visible = 1.0;
        for blocker in &self.occluders {
            let offset = blocker.center - point;
            let along = offset.dot(&direction);
            if along <= 0.0 || along >= distance {
                continue;
            }
            let blocker_distance = offset.norm();
            if blocker_distance <= blocker.radius {
                return 0.0;
            }
            let angular_blocker = (blocker.radius / blocker_distance)
                .clamp(0.0, 0.9999)
                .asin();
            let separation = (along / blocker_distance).clamp(-1.0, 1.0).acos();
            let covered = disc_overlap(angular_sun, angular_blocker, separation)
                / (PI * angular_sun * angular_sun);
            visible *= (1.0 - covered).clamp(0.0, 1.0);
            if visible < 0.001 {
                return 0.0;
            }
        }
        visible
    }

    pub fn shade(
        &self,
        point: Vec3,
        normal: Vec3,
        eye: Vec3,
        base: Color,
        hull_blocked: bool,
    ) -> Color {
        let relative = self.position - point;
        let distance = relative.norm().max(0.001);
        let light = relative / distance;
        let view = (eye - point).normalize();
        let normal = if normal.dot(&view) < 0.0 {
            -normal
        } else {
            normal
        };
        let diffuse = normal.dot(&light).max(0.0);
        let visible = if hull_blocked || diffuse == 0.0 {
            0.0
        } else {
            self.visibility(point)
        };
        let irradiance = (800.0 / distance.max(self.radius * 1.05))
            .powi(2)
            .clamp(0.08, 2.5);
        let direct = visible * irradiance;
        let half = light + view;
        let specular = if half.norm_squared() > 1e-8 {
            normal.dot(&half.normalize()).max(0.0).powf(48.0) * 0.45 * direct
        } else {
            0.0
        };
        // La luz ambiental es tenue; la sombra elimina la iluminacion solar directa.
        let channel = |value: u8, ambient: f32, warmth: f32| {
            (value as f32 * (ambient + diffuse * direct * warmth) + 255.0 * specular * warmth)
                .clamp(0.0, 255.0)
                .round() as u8
        };
        Color::new(
            channel(base.r, 0.065, 1.0),
            channel(base.g, 0.075, 0.90),
            channel(base.b, 0.10, 0.76),
        )
    }
}

pub struct HullShadow {
    tree: PlanetCollider,
}

impl HullShadow {
    pub fn new(model: &Model) -> Self {
        Self {
            tree: PlanetCollider::new_opaque(
                "Casco",
                model,
                WorldTransform {
                    position: Vec3::zeros(),
                    radius: 1.0,
                    yaw: 0.0,
                },
                0.0,
            ),
        }
    }

    pub fn occludes(&self, point: Vec3, sun: Vec3) -> bool {
        let ray = sun - point;
        if ray.norm_squared() < 1e-8 {
            return false;
        }
        self.tree
            .blocks_model_light(point + ray.normalize() * 0.003, sun)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exhaust_illuminates_the_rear_hull_with_warm_light_as_speed_increases() {
        let lights = ShipLights {
            accent: Color::new(142, 179, 250),
            exhausts: vec![Vec3::new(0.0, 0.0, -0.6)],
        };
        let shade = |power| {
            lights.shade(
                Color::new(0, 0, 0),
                Color::new(45, 45, 45),
                Vec3::new(0.0, 0.0, -0.58),
                -Vec3::z(),
                -Vec3::z(),
                power,
            )
        };
        let idle = shade(0.0);
        let moving = shade(1.0);
        let boost = shade(3.0);
        assert!(moving.r > idle.r && boost.r > moving.r);
        assert!(moving.r - idle.r > moving.b - idle.b);
    }

    #[test]
    fn sunlight_has_umbra_penumbra_and_distance_attenuation() {
        let light = SolarLight {
            position: Vec3::zeros(),
            radius: 130.0,
            occluders: vec![Occluder {
                center: Vec3::new(0.0, 0.0, 500.0),
                radius: 90.0,
            }],
        };
        assert_eq!(light.visibility(Vec3::new(0.0, 0.0, 700.0)), 0.0);
        assert!(light.visibility(Vec3::new(400.0, 0.0, 700.0)) > 0.99);
        assert!((1..300).any(|x| {
            let visible = light.visibility(Vec3::new(x as f32, 0.0, 700.0));
            visible > 0.05 && visible < 0.95
        }));
        let clear = SolarLight {
            position: Vec3::zeros(),
            radius: 130.0,
            occluders: vec![],
        };
        let base = Color::new(150, 150, 150);
        let near = clear.shade(
            Vec3::new(0.0, 0.0, 600.0),
            -Vec3::z(),
            Vec3::zeros(),
            base,
            false,
        );
        let far = clear.shade(
            Vec3::new(0.0, 0.0, 2200.0),
            -Vec3::z(),
            Vec3::zeros(),
            base,
            false,
        );
        let shadow = light.shade(
            Vec3::new(0.0, 0.0, 700.0),
            -Vec3::z(),
            Vec3::zeros(),
            base,
            false,
        );
        assert!(near.r > far.r && shadow.r < 20 && shadow.b < 20);
    }

    #[test]
    fn hull_shadow_uses_the_mesh_and_does_not_block_empty_space() {
        let model = Model::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/uv-checker.obj"
        ))
        .unwrap();
        let hull = HullShadow::new(&model);
        assert!(hull.occludes(Vec3::new(-0.1, -0.1, 1.0), Vec3::new(-0.1, -0.1, -2.0)));
        assert!(!hull.occludes(Vec3::new(5.0, 5.0, 1.0), Vec3::new(5.0, 5.0, -2.0)));
    }

    #[test]
    fn glass_does_not_cast_an_opaque_shadow_over_the_cockpit() {
        let mut model = Model::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/uv-checker.obj"
        ))
        .unwrap();
        model.faces[0].opacity = 0.22;
        let hull = HullShadow::new(&model);
        assert!(!hull.occludes(Vec3::new(-0.1, -0.1, 1.0), Vec3::new(-0.1, -0.1, -2.0)));
        // The same geometry still participates in physical collisions.
        let collider = PlanetCollider::new(
            "Glass",
            &model,
            WorldTransform {
                position: Vec3::zeros(),
                radius: 1.0,
                yaw: 0.0,
            },
            0.0,
        );
        assert!(
            collider.blocks_model_light(Vec3::new(-0.1, -0.1, 1.0), Vec3::new(-0.1, -0.1, -2.0))
        );
    }
}
