use crate::{color::Color, noise, texture::Texture};
use nalgebra_glm::{Vec2, Vec3};
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, Default)]
pub enum SurfaceEffect {
    #[default]
    None,
    Ice,
    Obsidian,
    Crystal,
    Snow {
        seconds: f32,
    },
    Stellar {
        seconds: f32,
    },
    Kepler {
        seconds: f32,
    },
    Clouds {
        seconds: f32,
        seed: f32,
        flow: f32,
    },
    Lava {
        seconds: f32,
    },
    Solar {
        seconds: f32,
    },
    PolarClouds {
        seconds: f32,
    },
    Rock {
        seconds: f32,
        seed: f32,
    },
}

fn sphere_direction(uv: Vec2) -> (Vec3, f32) {
    let longitude = uv.x * TAU;
    let latitude = uv.y * PI;
    let (sin_latitude, cos_latitude) = latitude.sin_cos();
    let (sin_longitude, cos_longitude) = longitude.sin_cos();
    (
        Vec3::new(
            sin_latitude * cos_longitude,
            cos_latitude,
            sin_latitude * sin_longitude,
        ),
        sin_latitude.max(0.0),
    )
}

pub fn kepler_flow(uv: Vec2, seconds: f32) -> (Vec2, f32) {
    let (point, pole_weight) = sphere_direction(uv);
    let drift = Vec3::new(seconds * 0.085, -seconds * 0.035, seconds * 0.055);
    let broad = noise::noise(point * 4.0 + drift);
    let detail = noise::noise(point * 11.0 - drift * 1.7 + Vec3::new(7.0, 3.0, 13.0));
    // El flujo modifica las UV por fragmento y se suaviza en los polos de la esfera.
    let zonal_wind = (point.y * 9.0 + seconds * 0.18).sin() * 0.007;
    let u = uv.x + seconds * 0.0015 + pole_weight * ((broad - 0.5) * 0.045 + zonal_wind);
    let v = uv.y + pole_weight * ((detail - 0.5) * 0.022 + (broad - 0.5) * 0.01);
    (
        Vec2::new(u.rem_euclid(1.0), v.clamp(0.0, 1.0)),
        0.92 + detail * 0.14,
    )
}

fn polar_flow(uv: Vec2, seconds: f32) -> (Vec2, f32) {
    // El centro y los semiejes corresponden a la elipse del generador de Jovara.
    let center = Vec2::new(498.5 / 996.0, 1.0 - 652.5 / 1279.0);
    let axes = Vec2::new(490.0 / 996.0, 518.0 / 1279.0);
    let offset = (uv - center).component_div(&axes);
    let original_radius = offset.norm();
    let radius = original_radius.min(1.0);
    let turbulence = noise::noise(Vec3::new(offset.x * 6.0, offset.y * 6.0, seconds * 0.28));
    let angle = seconds * 0.045 + (turbulence - 0.5) * 0.55;
    let (sin, cos) = angle.sin_cos();
    // La variacion radial se anula en el borde para no muestrear los margenes negros.
    let animated_radius = radius + (turbulence - 0.5) * 0.12 * radius * (1.0 - radius);
    let rotated = Vec2::new(
        offset.x * cos - offset.y * sin,
        offset.x * sin + offset.y * cos,
    ) * (animated_radius / original_radius.max(1e-8));
    (
        center + rotated.component_mul(&axes),
        0.90 + turbulence * 0.22,
    )
}

impl SurfaceEffect {
    pub fn modulate(self, color: Color, point: Vec3) -> Color {
        match self {
            Self::Crystal => {
                let grain = noise::noise(point * 37.0);
                if point.norm() < 1.045 {
                    let plates = noise::noise(point * 7.0 + Vec3::new(19.0, 5.0, 31.0));
                    let fracture = (point.x * 41.0 + point.z * 23.0 + plates * 4.0).sin().abs();
                    let edge = (1.0 - noise::smoothstep(0.015, 0.06, fracture)) * 0.15;
                    let base = color.shaded(0.85 + grain * 0.12 + plates * 0.15);
                    let lift = |v: u8, tint: f32| (v as f32 + edge * tint).min(255.0) as u8;
                    return Color::new(lift(base.r, 45.0), lift(base.g, 75.0), lift(base.b, 110.0));
                }
                let striation = (point.y * 173.0 + grain * 2.0).sin().abs();
                let glint = noise::smoothstep(0.94, 0.995, striation) * 0.10;
                let base = color.shaded(0.92 + grain * 0.12);
                let lift =
                    |channel: u8, tint: f32| (channel as f32 + glint * tint).min(255.0) as u8;
                Color::new(lift(base.r, 35.0), lift(base.g, 45.0), lift(base.b, 60.0))
            }
            Self::Snow { seconds } => {
                let grain = noise::noise(point * 145.0);
                let crust = noise::noise(point * 51.0);
                let wind = noise::noise(point * 22.0 + Vec3::new(seconds * 0.045, 7.0, 19.0));
                color.shaded(0.76 + grain * 0.30 + crust * 0.13 + (wind - 0.5) * 0.025)
            }
            Self::Obsidian => {
                let grain = noise::noise(point * 63.0);
                let frost = noise::smoothstep(
                    0.64,
                    0.82,
                    noise::noise(point * 17.0 + Vec3::new(13.0, 5.0, 29.0)),
                ) * 0.35;
                let base = color.shaded(0.9 + grain * 0.18);
                let mix = |v: u8, ice: f32| (v as f32 * (1.0 - frost) + ice * frost) as u8;
                Color::new(mix(base.r, 62.0), mix(base.g, 102.0), mix(base.b, 123.0))
            }
            Self::Stellar { seconds } => {
                let drift = Vec3::new(seconds * 0.18, -seconds * 0.11, seconds * 0.08);
                let granules = noise::noise(point * 43.0 + drift);
                let cells = noise::noise(point * 9.0 - drift * 0.7);
                let heat = noise::smoothstep(0.23, 0.78, granules * 0.65 + cells * 0.35);
                Color::new(
                    (145.0 + 110.0 * heat) as u8,
                    (188.0 + 67.0 * heat) as u8,
                    255,
                )
            }
            Self::Ice => {
                let grain = noise::noise(point * 95.0);
                if (point.normalize() - crate::ice_planet::lake_axis()).norm() < 0.35 {
                    // Irregular frozen plates: borders between the nearest Voronoi sites.
                    let p = Vec2::new(point.x, point.y) * 22.0;
                    let cell = p.map(|v| v.floor() as i32);
                    let mut closest = [f32::INFINITY; 2];
                    for y in -1..=1 {
                        for x in -1..=1 {
                            let (cx, cy) = (cell.x + x, cell.y + y);
                            let site = Vec2::new(
                                cx as f32 + noise::hash(cx, cy, 17),
                                cy as f32 + noise::hash(cx, cy, 31),
                            );
                            let distance = (p - site).norm_squared();
                            if distance < closest[0] {
                                closest = [distance, closest[0]];
                            } else if distance < closest[1] {
                                closest[1] = distance;
                            }
                        }
                    }
                    let frost =
                        (1.0 - noise::smoothstep(0.01, 0.045, closest[1] - closest[0])) * 0.65;
                    let mix =
                        |base: u8, ice: f32| (base as f32 * (1.0 - frost) + ice * frost) as u8;
                    Color::new(
                        mix(color.r, 175.0),
                        mix(color.g, 218.0),
                        mix(color.b, 231.0),
                    )
                    .shaded(0.94 + grain * 0.10)
                } else {
                    color.shaded(0.96 + grain * 0.06)
                }
            }
            Self::Rock { seconds, seed } => {
                let value =
                    noise::noise(point * 22.0 + Vec3::new(seed, seconds * 0.045, -seconds * 0.025));
                color.shaded(0.88 + value * 0.20)
            }
            _ => color,
        }
    }

    pub fn sample(self, texture: &Texture, uv: Vec2) -> Color {
        match self {
            Self::None => texture.sample(uv),
            Self::Crystal => self.modulate(texture.sample(uv), sphere_direction(uv).0),
            Self::Snow { .. } => self.modulate(texture.sample(uv), sphere_direction(uv).0),
            Self::Obsidian => self.modulate(texture.sample(uv), sphere_direction(uv).0),
            Self::Stellar { .. } => self.modulate(texture.sample(uv), sphere_direction(uv).0),
            Self::Ice => self.modulate(texture.sample(uv), sphere_direction(uv).0),
            Self::Kepler { seconds } => {
                let (coordinates, intensity) = kepler_flow(uv, seconds);
                texture.sample(coordinates).shaded(intensity)
            }
            Self::PolarClouds { seconds } => {
                let (coordinates, intensity) = polar_flow(uv, seconds);
                texture.sample(coordinates).shaded(intensity)
            }
            Self::Clouds {
                seconds,
                seed,
                flow,
            } => {
                let (point, pole_weight) = sphere_direction(uv);
                let turbulence =
                    noise::noise(point * 6.0 + Vec3::new(seed, seconds * 0.06, seconds * 0.04));
                let drift = pole_weight * (turbulence - 0.5) * flow;
                let longitude = if flow == 0.0 {
                    uv.x.clamp(0.0, 1.0)
                } else {
                    (uv.x + drift).rem_euclid(1.0)
                };
                let coordinates = Vec2::new(longitude, (uv.y + drift * 0.3).clamp(0.0, 1.0));
                texture.sample(coordinates).shaded(0.88 + turbulence * 0.22)
            }
            Self::Lava { seconds } => {
                let (coordinates, _) = kepler_flow(uv, seconds * 2.5);
                let base = texture.sample(coordinates);
                let (point, _) = sphere_direction(uv);
                let pulse =
                    noise::noise(point * 16.0 + Vec3::new(seconds * 0.16, 4.0, -seconds * 0.1));
                let hot = ((base.r as f32 - base.b as f32) / 160.0).clamp(0.0, 1.0);
                base.shaded(0.88 + (pulse - 0.25) * hot * 0.85)
            }
            Self::Solar { seconds } => {
                let (coordinates, intensity) = kepler_flow(uv, seconds * 3.0);
                let (point, _) = sphere_direction(uv);
                let convection =
                    noise::noise(point * 12.0 + Vec3::new(seconds * 0.40, -seconds * 0.22, 9.0));
                texture
                    .sample(coordinates)
                    .shaded(intensity * (0.78 + convection * 0.48))
            }
            Self::Rock { .. } => {
                let (point, _) = sphere_direction(uv);
                self.modulate(texture.sample(uv), point)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jovara_flow_stays_inside_the_photograph_at_every_angle_and_keeps_animating() {
        let texture = Texture::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/planets/textures/jovara-clouds.png"
        ))
        .unwrap();
        let center = Vec2::new(498.5 / 996.0, 1.0 - 652.5 / 1279.0);
        let axes = Vec2::new(490.0 / 996.0, 518.0 / 1279.0);
        let mut changed = 0;
        for seconds in [
            0.0, 2.0, 10.0, 20.0, 35.0, 60.0, 90.0, 140.0, 300.0, 600.0, 3600.0,
        ] {
            for ring in 0..=10 {
                for angle in 0..128 {
                    let angle = angle as f32 * TAU / 128.0;
                    let offset = Vec2::new(angle.cos(), angle.sin()) * (ring as f32 / 10.0);
                    let uv = center + offset.component_mul(&axes);
                    let (sample, _) = polar_flow(uv, seconds);
                    assert!((sample - center).component_div(&axes).norm() <= 1.000001);
                    let color = SurfaceEffect::PolarClouds { seconds }.sample(&texture, uv);
                    assert!(
                        color.r.max(color.g).max(color.b) > 15,
                        "Margen negro en {uv:?}, t={seconds}"
                    );
                    if color
                        != (SurfaceEffect::PolarClouds {
                            seconds: seconds + 2.0,
                        })
                        .sample(&texture, uv)
                    {
                        changed += 1;
                    }
                }
            }
        }
        assert!(changed > 1000);
        assert_eq!(polar_flow(center, 60.0).0, center);
    }

    #[test]
    fn cloud_lava_and_rock_noise_animate_without_replacing_the_base_material() {
        let texture = Texture::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/planets/textures/aurora-flareon-surface.png"
        ))
        .unwrap();
        let effects = |seconds| {
            [
                SurfaceEffect::Clouds {
                    seconds,
                    seed: 13.0,
                    flow: 0.0,
                },
                SurfaceEffect::Clouds {
                    seconds,
                    seed: 37.0,
                    flow: 0.018,
                },
                SurfaceEffect::Lava { seconds },
                SurfaceEffect::Rock {
                    seconds,
                    seed: 43.0,
                },
            ]
        };
        for (before, after) in effects(0.0).into_iter().zip(effects(8.0)) {
            let changed = (1..20)
                .flat_map(|x| (1..20).map(move |y| Vec2::new(x as f32 / 20.0, y as f32 / 20.0)))
                .filter(|&uv| before.sample(&texture, uv) != after.sample(&texture, uv))
                .count();
            assert!(changed > 40);
        }
    }

    #[test]
    fn kepler_noise_animates_reproducibly_and_closes_the_longitude_seam() {
        let uv = Vec2::new(0.37, 0.58);
        let before = kepler_flow(uv, 0.0);
        assert_eq!(before, kepler_flow(uv, 0.0));
        assert!((before.0 - kepler_flow(uv, 4.0).0).norm() > 0.003);
        for seconds in [0.0, 4.0, 120.0] {
            for v in [0.0, 0.3, 0.5, 0.8, 1.0] {
                let a = kepler_flow(Vec2::new(0.0, v), seconds);
                let b = kepler_flow(Vec2::new(1.0, v), seconds);
                let longitude_error = (a.0.x - b.0.x).abs();
                assert!(longitude_error.min(1.0 - longitude_error) < 0.00001);
                assert!((a.0.y - b.0.y).abs() < 0.00001);
                assert!((a.1 - b.1).abs() < 0.00001);
            }
        }
    }
}
