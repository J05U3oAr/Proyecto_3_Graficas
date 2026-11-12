use crate::{color::Color, noise, texture::Texture};
use nalgebra_glm::{Vec2, Vec3};
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, Default)]
pub enum SurfaceEffect {
    #[default]
    None,
    Ice {
        seconds: f32,
        light: Vec3,
        eye: Vec3,
        normal: Vec3,
        detail: bool,
        core_radius: f32,
    },
    Obsidian,
    Asteroid,
    Crystal,
    GasGiant {
        seconds: f32,
        light: Vec3,
        eye: Vec3,
    },
    RingIce,
    PackIce {
        seconds: f32,
    },
    Snow {
        seconds: f32,
    },
    Stellar {
        seconds: f32,
    },
    GoldenStar {
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
    CrimsonLava {
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

fn ice_surface(effect: SurfaceEffect, base: Color, p: Vec3) -> Color {
    let SurfaceEffect::Ice {
        seconds,
        light: sun,
        eye,
        normal: geometric_normal,
        detail,
        core_radius,
    } = effect
    else {
        return base;
    };
    let radial = p.normalize();
    let geometric_normal = geometric_normal.normalize();
    let broad = noise::noise(p * 5.0 + Vec3::new(4.0, 17.0, 26.0));
    let strata = noise::noise(p * 23.0 + Vec3::new(11.0, 3.0, 7.0));
    let (grain, gradient) = if detail {
        noise::noise_with_gradient(p * 9.0 + Vec3::new(7.0, 19.0, 3.0))
    } else {
        (strata, Vec3::zeros())
    };
    // Broad, low-contrast variations keep the snow clean instead of granular.
    let snow = Vec3::new(202.0, 210.0, 213.0) * (0.97 + broad * 0.035);
    let mut glacial = Vec3::new(97.0, 122.0, 137.0) + Vec3::new(18.0, 20.0, 21.0) * broad;
    // Lake faces use the continuous shoreline mask below, not their triangle edges.
    let ice_weight = if (70..=160).contains(&base.r) {
        1.0
    } else {
        0.0
    };
    let deposition = if ice_weight == 0.0 {
        1.0
    } else {
        noise::smoothstep(
            0.67,
            0.96,
            geometric_normal.dot(&radial) * 0.80 + broad * 0.20,
        )
    };
    glacial += (snow - glacial) * deposition * 0.60;
    let mut albedo = snow + (glacial - snow) * ice_weight;
    let lake_distance = (radial - crate::ice_planet::lake_axis()).norm();
    // A defined, irregular shoreline separates opaque snow from opaque ice.
    // Only a narrow antialiasing fringe softens the contour, not the whole bank.
    let shore_distance = lake_distance + (broad - 0.5) * 0.04 + (grain - 0.5) * 0.03;
    let lake = 1.0 - noise::smoothstep(0.321, 0.324, shore_distance);
    if lake > 0.0 {
        let axis = crate::ice_planet::lake_axis();
        let right = axis.cross(&Vec3::y()).normalize();
        let up = right.cross(&axis).normalize();
        let uv = Vec2::new(p.dot(&right), p.dot(&up)) * 19.0
            + Vec2::new(broad - 0.5, strata - 0.5) * 0.45;
        let cell = uv.map(|v| v.floor() as i32);
        let mut nearest = [f32::INFINITY; 2];
        for y in -1..=1 {
            for x in -1..=1 {
                let (cx, cy) = (cell.x + x, cell.y + y);
                let site = Vec2::new(
                    cx as f32 + noise::hash(cx, cy, 17),
                    cy as f32 + noise::hash(cx, cy, 31),
                );
                let distance = (uv - site).norm_squared();
                if distance < nearest[0] {
                    nearest = [distance, nearest[0]];
                } else if distance < nearest[1] {
                    nearest[1] = distance;
                }
            }
        }
        let fracture = (1.0 - noise::smoothstep(0.015, 0.070, nearest[1] - nearest[0]))
            * noise::smoothstep(0.42, 0.70, strata);
        let mut frozen = Vec3::new(37.0, 62.0, 75.0) + Vec3::new(12.0, 18.0, 24.0) * broad;
        frozen += (Vec3::new(142.0, 162.0, 171.0) - frozen) * fracture * 0.65;
        // A thin, opaque frost shelf softens the contact without fading the lake.
        let frost = noise::smoothstep(0.312, 0.314, shore_distance);
        frozen += (Vec3::new(91.0, 115.0, 127.0) - frozen) * frost * 0.65;
        albedo += (frozen - albedo) * lake;
    }
    // Only loose powder moves; the ice, fractures and baked terrain remain fixed.
    if detail {
        let wind = noise::noise(
            Vec3::new(p.x * 9.0, p.y * 27.0, p.z * 9.0)
                + Vec3::new(seconds * 0.016, 7.0, -seconds * 0.011),
        );
        let veil = noise::smoothstep(0.48, 0.78, wind) * deposition * (0.035 + lake * 0.025);
        albedo += (Vec3::new(215.0, 222.0, 225.0) - albedo) * veil;
    }
    // Snow follows the smooth sphere; the flat lake follows its own plane.
    // Glacier walls retain their modeled relief instead of rounding the obstacles.
    let surface_normal = radial + (geometric_normal - radial) * ice_weight;
    let surface_normal =
        (surface_normal + (crate::ice_planet::lake_axis() - surface_normal) * lake).normalize();
    let tangent_gradient = gradient - surface_normal * gradient.dot(&surface_normal);
    // Rounded snowdrifts use broad normal variation, without fine surface grain.
    // The lake stays flat and glacier walls keep their subtler ice finish.
    // Snowdrifts flatten gradually before reaching the shore, so they do not
    // form a raised rim around the planar ice.
    let snow_relief = noise::smoothstep(0.28, 0.48, shore_distance);
    let bump = (0.32 * (1.0 - ice_weight) * snow_relief + ice_weight * 0.018) * (1.0 - lake)
        + lake * 0.004;
    // Round only the snow lip through lighting, keeping its color boundary sharp.
    let lip =
        (1.0 - noise::smoothstep(0.324, 0.342, shore_distance)) * (1.0 - lake) * (1.0 - ice_weight);
    let bank_direction =
        radial * radial.dot(&crate::ice_planet::lake_axis()) - crate::ice_planet::lake_axis();
    let bank_direction = bank_direction
        .try_normalize(0.00001)
        .unwrap_or(Vec3::zeros());
    let normal =
        (surface_normal - tangent_gradient * bump - bank_direction * lip * 0.18).normalize();
    let light = (sun - p).normalize();
    let view = (eye - p).normalize();
    // The solid planetary core hides the sun from back-facing glacier walls.
    let toward_center = -p.dot(&light);
    let visibility = if toward_center > 0.0 {
        let closest = p.norm_squared() - toward_center * toward_center;
        noise::smoothstep(
            core_radius * core_radius * 0.94,
            core_radius * core_radius * 1.04,
            closest,
        )
    } else {
        1.0
    };
    let diffuse = normal.dot(&light).max(0.0) * visibility;
    let half = (light + view).try_normalize(0.00001).unwrap_or(normal);
    let facing = normal.dot(&view).max(0.0);
    let fresnel = 0.035 + 0.965 * (1.0 - facing).powi(5);
    let surface_gloss = if ice_weight > 0.5 {
        normal.dot(&half).max(0.0).powi(48) * 0.40
    } else {
        normal.dot(&half).max(0.0).powi(16) * 0.045
    };
    let lake_gloss = normal.dot(&half).max(0.0).powi(96) * 0.85;
    let gloss = surface_gloss + (lake_gloss - surface_gloss) * lake;
    let highlight = gloss * (0.12 + fresnel) * diffuse;
    let haze = (1.0 - radial.dot(&view).max(0.0)).powi(3) * diffuse * 0.045;
    let result = albedo * (0.055 + diffuse * 0.945)
        + Vec3::new(224.0, 234.0, 239.0) * highlight
        + Vec3::new(82.0, 106.0, 121.0) * haze;
    Color::new(
        result.x.clamp(0.0, 255.0) as u8,
        result.y.clamp(0.0, 255.0) as u8,
        result.z.clamp(0.0, 255.0) as u8,
    )
}

impl SurfaceEffect {
    pub fn modulate(self, color: Color, point: Vec3) -> Color {
        match self {
            Self::GoldenStar { seconds } => {
                let drift = Vec3::new(seconds * 0.08, -seconds * 0.04, seconds * 0.06);
                let cells = noise::noise(point * 28.0 + drift);
                let filaments = noise::noise(point * 83.0 - drift * 1.4);
                let heat = noise::smoothstep(0.18, 0.85, cells * 0.75 + filaments * 0.25);
                let spots = noise::smoothstep(0.68, 0.83, noise::noise(point * 7.0 + drift * 0.14));
                Color::new(
                    255,
                    (103.0 + heat * 141.0) as u8,
                    (12.0 + heat * 91.0) as u8,
                )
                .shaded(1.0 - spots * 0.65)
            }
            Self::Asteroid => {
                let grain = noise::noise(point * 97.0);
                color.shaded(0.84 + grain * 0.25)
            }
            Self::RingIce => {
                let tilt = 27_f32.to_radians();
                let radial =
                    Vec2::new(point.x, -point.y * tilt.sin() + point.z * tilt.cos()).norm();
                let grain = noise::noise(point * 173.0);
                let narrow = (radial * 1700.0).sin() * 0.5 + 0.5;
                let broad = noise::noise(Vec3::new(radial * 215.0, 7.0, 3.0));
                color.shaded(0.30 + broad * 0.48 + narrow * 0.25 + grain * 0.12)
            }
            Self::GasGiant {
                seconds,
                light,
                eye,
            } => {
                let p = point.normalize();
                let drift = Vec3::new(seconds * 0.007, 0.0, -seconds * 0.005);
                let flow = noise::noise(p * 4.2 + drift);
                let clouds = Vec3::new(p.x * 7.0, p.y * 35.0, p.z * 7.0)
                    + Vec3::new(flow * 2.0, flow * 2.0, -flow * 1.5)
                    + drift;
                let mut storm = 0.0_f32;
                let mut storm_density = 0.0;
                for (center, width, height, spin) in [
                    (Vec3::new(0.25, 0.30, 0.9206), 0.38, 0.19, 1.0),
                    (Vec3::new(-0.87, -0.26, 0.42), 0.26, 0.14, -1.0),
                    (Vec3::new(0.50, -0.67, -0.55), 0.31, 0.17, 1.0),
                    (Vec3::new(-0.44, 0.48, -0.759), 0.35, 0.18, -1.0),
                ] {
                    let center = center.normalize();
                    let delta = p - center;
                    if delta.norm_squared() > 0.24 {
                        continue;
                    }
                    let right = center.cross(&Vec3::y()).normalize();
                    let up = right.cross(&center).normalize();
                    let x = delta.dot(&right) / width;
                    let y = delta.dot(&up) / height;
                    let r = (x * x + y * y).sqrt();
                    if r >= 1.25 {
                        continue;
                    }
                    // Twist the sampling coordinates instead of drawing concentric rings.
                    // The original cloud field forms the spiral and its broken wispy edges.
                    let envelope = 1.0 - noise::smoothstep(0.10, 1.25, r);
                    let twist = envelope * (3.8 + flow * 0.8) * spin + seconds * 0.025 * spin;
                    let (sin, cos) = twist.sin_cos();
                    let curl = Vec3::new(
                        (x * cos - y * sin) * 3.2,
                        (x * sin + y * cos) * 4.5,
                        13.0 + flow * 0.6,
                    );
                    let billows = noise::noise(curl);
                    let wisps = noise::noise(curl * 3.1 + Vec3::new(7.0, 3.0, 19.0));
                    if envelope > storm {
                        storm = envelope;
                        storm_density = billows * 0.70 + wisps * 0.30;
                    }
                }
                let large = noise::noise(clouds);
                let medium = noise::noise(clouds * 2.13 + Vec3::new(7.0, 13.0, 3.0));
                let fine = noise::noise(clouds * 5.7 + Vec3::new(17.0, 3.0, 29.0));
                let background = large * 0.70 + medium * 0.22 + fine * 0.08;
                // Blend densities, never unrelated coordinate fields: that creates contour rings.
                let density = background + (storm_density - background) * storm;
                let navy = Vec3::new(10.0, 25.0, 48.0);
                let slate = Vec3::new(64.0, 89.0, 111.0);
                let white = Vec3::new(218.0, 229.0, 235.0);
                let mut palette = navy + (slate - navy) * noise::smoothstep(0.27, 0.73, density);
                let cloud_cover =
                    noise::smoothstep(0.48 - storm * 0.10, 0.72 - storm * 0.08, density);
                palette += (white - palette) * cloud_cover * (0.28 + storm * 0.72);
                // Analytic oblate normal gives continuous diffuse lighting, without plastic specular.
                let normal = Vec3::new(p.x, p.y / (0.93 * 0.93), p.z).normalize();
                let incidence = normal.dot(&light);
                let diffuse = noise::smoothstep(-0.08, 0.85, incidence);
                let limb = (1.0 - normal.dot(&eye).max(0.0)).powi(3);
                let scatter = Vec3::new(18.0, 39.0, 66.0)
                    * limb
                    * noise::smoothstep(-0.18, 0.25, incidence)
                    * 0.38;
                let result = palette * (0.07 + diffuse * 0.93) + scatter;
                Color::new(
                    result.x.clamp(0.0, 255.0) as u8,
                    result.y.clamp(0.0, 255.0) as u8,
                    result.z.clamp(0.0, 255.0) as u8,
                )
            }
            Self::PackIce { seconds } => {
                let grain = noise::noise(point * 83.0);
                if color.r < 80 && color.b as f32 > color.r as f32 * 1.65 {
                    let drift = Vec3::new(seconds * 0.035, 11.0, -seconds * 0.025);
                    let shimmer = noise::noise(point * 31.0 + drift);
                    return color.shaded(0.86 + shimmer * 0.24);
                }
                let compression = noise::noise(point * 21.0 + Vec3::new(9.0, 17.0, 3.0));
                color.shaded(0.94 + grain * 0.07 + compression * 0.04)
            }
            Self::Crystal => {
                let grain = noise::noise(point * 37.0);
                if point.norm() < 1.045 {
                    // Like nebula.rs: domain warping, layered fBm and continuous RGB mixing.
                    // A neutral ground material carries face lighting into this palette.
                    let p = point.normalize();
                    let warp = noise::noise(p * 3.2 + Vec3::new(7.0, 2.0, 5.0)) - 0.5;
                    let clouds = noise::fbm(p * 5.0 + Vec3::new(15.0 + warp, 3.0, -8.0));
                    let wisps = noise::fbm(p * 13.0 + Vec3::new(4.0, 23.0 + warp * 2.0, 6.0));
                    let hue = noise::smoothstep(
                        0.27,
                        0.72,
                        noise::fbm(p * 2.5 + Vec3::new(19.0, 5.0, 31.0)) + (clouds - 0.5) * 0.5,
                    );
                    let navy = Vec3::new(14.0, 29.0, 78.0);
                    let blue = Vec3::new(26.0, 107.0, 179.0);
                    let purple = Vec3::new(118.0, 53.0, 174.0);
                    let tint = blue + (purple - blue) * hue;
                    let density = 0.35 + noise::smoothstep(0.26, 0.76, clouds) * 0.65;
                    let palette = navy + (tint - navy) * density;
                    let light = (color.r as f32 + color.g as f32 + color.b as f32) / (3.0 * 255.0);
                    let result = palette * light * (0.85 + wisps * 0.30);
                    return Color::new(
                        result.x.clamp(0.0, 255.0) as u8,
                        result.y.clamp(0.0, 255.0) as u8,
                        result.z.clamp(0.0, 255.0) as u8,
                    );
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
                // LAB5-style layers: broad mineral variation, warped veins,
                // restrained rock detail, then frost. All stay fixed to the mesh.
                let mineral = noise::fbm(point * 4.0 + Vec3::new(13.0, 5.0, 29.0));
                let detail = noise::noise(point * 31.0 + Vec3::new(7.0, 19.0, 3.0));
                let veins = noise::noise(point * 18.0 + Vec3::repeat(mineral * 3.0));
                let fissures = 1.0 - noise::smoothstep(0.015, 0.055, (veins - 0.5).abs());
                let base =
                    color.shaded((0.83 + mineral * 0.24 + detail * 0.08) * (1.0 - fissures * 0.09));
                let frost = noise::smoothstep(0.58, 0.73, mineral + (detail - 0.5) * 0.045) * 0.20;
                let mix = |v: u8, ice: f32| (v as f32 * (1.0 - frost) + ice * frost) as u8;
                Color::new(mix(base.r, 62.0), mix(base.g, 94.0), mix(base.b, 111.0))
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
            Self::Ice { .. } => ice_surface(self, color, point),
            Self::Lava { seconds } | Self::CrimsonLava { seconds } => {
                let grain = noise::noise(point * 21.0 + Vec3::new(7.0, 3.0, 11.0));
                let pulse = if color.r > 170 {
                    (seconds * 2.0 + point.y * 9.0).sin() * 0.08
                } else {
                    0.0
                };
                color.shaded(0.86 + grain * 0.24 + pulse)
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
            Self::CrimsonLava { seconds } => {
                let base = Self::Lava { seconds }.sample(texture, uv);
                let heat = (base.r as f32 * 0.8 + base.g as f32 * 0.2) / 255.0;
                // La paleta conserva las fisuras animadas y separa la roca granate de la lava carmesi.
                let stops = [
                    (0.0, Color::new(20, 4, 11)),
                    (0.22, Color::new(75, 9, 30)),
                    (0.62, Color::new(184, 20, 43)),
                    (0.9, Color::new(255, 55, 42)),
                    (1.0, Color::new(255, 180, 132)),
                ];
                let pair = stops
                    .windows(2)
                    .find(|pair| heat <= pair[1].0)
                    .unwrap_or(&stops[3..]);
                let t = ((heat - pair[0].0) / (pair[1].0 - pair[0].0)).clamp(0.0, 1.0);
                let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
                Color::new(
                    mix(pair[0].1.r, pair[1].1.r),
                    mix(pair[0].1.g, pair[1].1.g),
                    mix(pair[0].1.b, pair[1].1.b),
                )
            }
            Self::None => texture.sample(uv),
            Self::GasGiant { .. } | Self::RingIce | Self::Asteroid => {
                self.modulate(texture.sample(uv), sphere_direction(uv).0)
            }
            Self::PackIce { .. } => self.modulate(texture.sample(uv), sphere_direction(uv).0),
            Self::Crystal => self.modulate(texture.sample(uv), sphere_direction(uv).0),
            Self::Snow { .. } => self.modulate(texture.sample(uv), sphere_direction(uv).0),
            Self::Obsidian => self.modulate(texture.sample(uv), sphere_direction(uv).0),
            Self::Stellar { .. } => self.modulate(texture.sample(uv), sphere_direction(uv).0),
            Self::GoldenStar { .. } => self.modulate(texture.sample(uv), sphere_direction(uv).0),
            Self::Ice { .. } => self.modulate(texture.sample(uv), sphere_direction(uv).0),
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
    fn nivalis_snow_is_smooth_across_mesh_faces() {
        let p = Vec3::new(0.3, 0.5, -0.7).normalize() * 0.83;
        let shade = |normal| {
            SurfaceEffect::Ice {
                seconds: 0.0,
                light: Vec3::new(1.0, 3.0, -4.0),
                eye: Vec3::new(0.0, 0.0, -3.0),
                normal,
                detail: true,
                core_radius: 0.79,
            }
            .modulate(Color::new(209, 232, 237), p)
        };
        let a = shade(Vec3::y());
        let b = shade(Vec3::z());
        assert_eq!(
            (a.r, a.g, a.b),
            (b.r, b.g, b.b),
            "La nieve no debe mostrar las caras triangulares de la malla"
        );
    }

    #[test]
    fn nivalis_layers_follow_sunlight_animate_powder_and_preserve_depth() {
        use crate::{
            flight::Flight,
            framebuffer::Framebuffer,
            renderer::{self, FlightView, WorldTransform},
        };
        let mut space = crate::space::Space::load().unwrap();
        space.configure(2);
        let planet = space
            .active_planets()
            .find(|p| p.name == "Nivalis")
            .unwrap();
        let mut flight = Flight::new();
        flight.camera.pitch = 0.0;
        let view = FlightView::new(&flight, 320, 240);
        let transform = WorldTransform {
            position: Vec3::new(0.0, 0.0, -260.0),
            radius: 45.0,
            yaw: 0.0,
        };
        let render = |seconds, sun| {
            let mut frame = Framebuffer::new(320, 240);
            frame.pixels.fill(0);
            frame.point(160, 120, 2.0, 0xff00ff, true);
            renderer::draw_world_model_shaded(
                &mut frame,
                &planet.model,
                &view,
                transform,
                Some(sun),
                planet.surface_effect(seconds),
            );
            frame
        };
        let day = render(0.0, Vec3::new(0.0, 100.0, 300.0));
        let moving = render(12.0, Vec3::new(0.0, 100.0, 300.0));
        let night = render(0.0, Vec3::new(0.0, 100.0, -900.0));
        assert_eq!(
            day.depth, moving.depth,
            "Las capas no deben cambiar el terreno ni sus colisiones"
        );
        assert_eq!(moving.pixels[120 * 320 + 160], 0xff00ff);
        let changed = day
            .pixels
            .iter()
            .zip(&moving.pixels)
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            changed > 100,
            "Debe apreciarse la deriva de nieve fina: {changed}"
        );
        let brightness = |frame: &Framebuffer| {
            frame
                .pixels
                .iter()
                .map(|p| ((p >> 16) & 255) as u64 + ((p >> 8) & 255) as u64 + (p & 255) as u64)
                .sum::<u64>()
        };
        assert!(
            brightness(&day) > brightness(&night) * 4,
            "El hielo no debe emitir luz en la sombra"
        );
    }

    #[test]
    fn crystal_ground_blends_blue_and_purple_continuously_in_object_space() {
        let mut blue = 0;
        let mut purple = 0;
        for i in 0..256 {
            let y = 1.0 - 2.0 * (i as f32 + 0.5) / 256.0;
            let angle = i as f32 * 2.3999632;
            let r = (1.0 - y * y).sqrt();
            let point = Vec3::new(r * angle.cos(), y, r * angle.sin());
            let a = SurfaceEffect::Crystal.modulate(Color::new(220, 220, 220), point);
            let b = SurfaceEffect::Crystal
                .modulate(Color::new(220, 220, 220), point + Vec3::x() * 0.0001);
            blue += usize::from(a.g > a.r + 10 && a.b > a.r + 35);
            purple += usize::from(a.r > a.g + 10 && a.b > a.r + 15);
            for (x, y) in [(a.r, b.r), (a.g, b.g), (a.b, b.b)] {
                assert!(
                    (x as i32 - y as i32).abs() < 10,
                    "La mezcla no debe tener saltos de color"
                );
            }
        }
        assert!(
            blue > 20 && purple > 20,
            "Debe mezclar azul y purpura: {blue}/{purple}"
        );
    }

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
