//! Nácar: opaque glacial ice, snow caps and dark leads between pack-ice plates.
use crate::{color::Color, noise};
use nalgebra_glm::Vec3;

fn water_fraction(color: Color) -> f32 {
    1.0 - noise::smoothstep(22.0, 55.0, color.r as f32)
}

pub fn albedo(color: Color, point: Vec3, seconds: f32, pixel_radius: f32) -> Color {
    let water = water_fraction(color);
    let compaction = noise::noise(point * 6.0 + Vec3::new(9.0, 17.0, 3.0));
    let strata = noise::noise(point * 18.0 + Vec3::new(compaction * 2.0, 7.0, 13.0));
    // Two continuous frequency bands add frost/ice detail without the square
    // plateaus of high-frequency value noise. Both stay anchored to the mesh.
    let detail = noise::smoothstep(80.0, 240.0, pixel_radius);
    let frost = if detail > 0.0 {
        noise::gradient_noise(point * 26.0 + Vec3::new(compaction * 2.4, strata * 1.6, compaction))
    } else {
        0.5
    };
    let fine = if water < 1.0 && detail > 0.0 {
        noise::gradient_noise(point * 57.0 + Vec3::new(frost * 2.0, 11.0, 7.0))
    } else {
        0.5
    };
    let solid = 0.975
        + compaction * 0.025
        + strata * 0.015
        + ((frost - 0.5) * 0.035 + (fine - 0.5) * 0.035) * detail;
    let liquid = if water > 0.0 {
        let waves = noise::noise(Vec3::new(
            point.x * 14.0 + seconds * 0.04,
            point.y * 7.0,
            point.z * 14.0 - seconds * 0.025,
        ));
        0.94 + waves * 0.10
    } else {
        1.0
    };
    // Solid layers stay fixed; only the narrow channels shimmer with time.
    color.shaded(solid + (liquid - solid) * water)
}

pub fn lighting(normal: Vec3, color: Color, light: &Vec3, half: &Vec3) -> Color {
    let n = normal.normalize();
    let incidence = n.dot(light);
    let diffuse = incidence.max(0.0);
    let water = water_fraction(color);
    let frost = noise::smoothstep(0.70, 0.92, color.r as f32 / (color.b as f32).max(1.0));
    // Wrapped diffuse approximates scattering through milky ice, rather than glass transparency.
    let wrapped = ((incidence + 0.35) / 1.35).max(0.0);
    let snow_light = 0.24 + diffuse * 0.76;
    let ice_light = 0.20 + diffuse * 0.58 + wrapped * 0.22;
    let solid_light = ice_light + (snow_light - ice_light) * frost;
    let intensity = solid_light + (0.12 + diffuse * 0.40 - solid_light) * water;
    let base = Vec3::new(color.r as f32, color.g as f32, color.b as f32) * intensity;
    let facing = n.z.abs().clamp(0.0, 1.0);
    let fresnel = 0.02 + 0.98 * (1.0 - facing).powi(5);
    let alignment = n.dot(half).max(0.0);
    let glint = alignment.powi(48) * (1.0 - frost) * 0.055 * diffuse
        + alignment.powi(140) * water * (0.10 + fresnel * 0.40) * diffuse;
    let reflection = Vec3::new(20.0, 47.0, 68.0) * fresnel * water * 0.55;
    let rgb = base + Vec3::new(219.0, 234.0, 243.0) * glint + reflection;
    Color::new(
        rgb.x.clamp(0.0, 255.0) as u8,
        rgb.y.clamp(0.0, 255.0) as u8,
        rgb.z.clamp(0.0, 255.0) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn solid_glacier_is_stable_while_channels_animate_and_snow_stays_matte() {
        let p = Vec3::new(0.28, 0.31, 0.7);
        for color in [Color::new(153, 204, 222), Color::new(230, 242, 250)] {
            assert_eq!(albedo(color, p, 0.0, 512.0), albedo(color, p, 20.0, 512.0));
        }
        let water = Color::new(7, 28, 59);
        assert!((0..50).any(|i| {
            let p = p + Vec3::x() * i as f32 * 0.02;
            albedo(water, p, 0.0, 512.0) != albedo(water, p, 20.0, 512.0)
        }));
        let snow = lighting(Vec3::z(), Color::new(230, 242, 250), &Vec3::z(), &Vec3::z());
        assert!(snow.r <= 231 && snow.g <= 243 && snow.b <= 251);
    }
}
