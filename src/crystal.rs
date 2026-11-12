//! Optical material for Prisma. Screen-space transmission complements the mesh pipeline.
use crate::{color::Color, noise};
use nalgebra_glm::{Vec2, Vec3};

const IOR: f32 = 1.54;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Ground,
    Core,
    Glass,
}

#[derive(Clone, Copy)]
pub struct Material {
    pub kind: Kind,
    pub tint: Color,
    pub opacity: f32,
}

impl Material {
    pub fn new(tint: Color, opacity: f32) -> Self {
        let kind = if opacity < 1.0 {
            Kind::Glass
        } else if tint == Color::new(255, 255, 255) {
            Kind::Ground
        } else {
            Kind::Core
        };
        Self {
            kind,
            tint,
            opacity,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Optics {
    pub eye: Vec3,
    pub light: Vec3,
    pub normal: Vec3,
    pub screen_right: Vec3,
    pub screen_up: Vec3,
    pub pixel_scale: f32,
    pub ground_radius: f32,
    pub sun_cosine: f32,
    pub rotation: Vec2,
}

pub struct Fragment {
    pub radiance: Vec3,
    pub transmission: Vec3,
    pub refraction: Vec2,
}

pub fn fresnel(cosine: f32) -> f32 {
    let f0 = ((IOR - 1.0) / (IOR + 1.0)).powi(2);
    f0 + (1.0 - f0) * (1.0 - cosine.clamp(0.0, 1.0)).powi(5)
}

fn absorption(tint: Color, path: f32) -> Vec3 {
    Vec3::new(tint.r as f32, tint.g as f32, tint.b as f32)
        .map(|v| (-((1.0 - v / 255.0).powi(2) * 4.0 + 0.04) * path).exp())
}

pub fn ground_palette(point: Vec3) -> Color {
    let p = point.normalize();
    let warp = noise::noise(p * 3.2 + Vec3::new(7.0, 2.0, 5.0)) - 0.5;
    let field = p * 5.0 + Vec3::new(15.0 + warp, 3.0, -8.0);
    let clouds =
        noise::noise(field) * 0.72 + noise::noise(field * 2.13 + Vec3::new(7.0, 13.0, 3.0)) * 0.28;
    let wisps = noise::noise(p * 24.0 + Vec3::new(4.0, 23.0 + warp * 2.0, 6.0));
    let hue = noise::smoothstep(
        0.27,
        0.72,
        noise::noise(p * 2.5 + Vec3::new(19.0, 5.0, 31.0)) + (clouds - 0.5) * 0.5,
    );
    let navy = Vec3::new(14.0, 29.0, 78.0);
    let tint = Vec3::new(26.0, 107.0, 179.0)
        + (Vec3::new(118.0, 53.0, 174.0) - Vec3::new(26.0, 107.0, 179.0)) * hue;
    let density = 0.35 + noise::smoothstep(0.26, 0.76, clouds) * 0.65;
    pack((navy + (tint - navy) * density) * (0.92 + wisps * 0.16))
}

pub fn pack(rgb: Vec3) -> Color {
    Color::new(
        rgb.x.clamp(0.0, 255.0) as u8,
        rgb.y.clamp(0.0, 255.0) as u8,
        rgb.z.clamp(0.0, 255.0) as u8,
    )
}

/// Compress strong stellar reflections before conversion to the framebuffer.
pub fn display(rgb: Vec3) -> Color {
    pack(rgb.map(|v| {
        if v > 180.0 {
            180.0 + 75.0 * (1.0 - (-(v - 180.0) / 90.0).exp())
        } else {
            v
        }
    }))
}

pub fn unpack(color: u32) -> Vec3 {
    Vec3::new(
        ((color >> 16) & 255) as f32,
        ((color >> 8) & 255) as f32,
        (color & 255) as f32,
    )
}

pub fn shade(material: Material, optics: Optics, p: Vec3) -> Fragment {
    let view = (optics.eye - p).normalize();
    let light = (optics.light - p).normalize();
    let n = if material.kind == Kind::Ground {
        let radial = p.normalize();
        let (_, gradient) = noise::noise_with_gradient(p * 7.0 + Vec3::new(3.0, 11.0, 7.0));
        (radial * 0.55 + optics.normal * 0.45 - (gradient - radial * gradient.dot(&radial)) * 0.008)
            .normalize()
    } else {
        optics.normal
    };
    let exiting = n.dot(&view) < 0.0;
    let normal = if exiting { -n } else { n };
    let cosine = normal.dot(&view).clamp(0.0, 1.0);
    let eta = if exiting { IOR } else { 1.0 / IOR };
    let k = 1.0 - eta * eta * (1.0 - cosine * cosine);
    let f = if material.kind == Kind::Glass && k < 0.0 {
        1.0
    } else {
        fresnel(cosine)
    };
    let reflected = normal * (2.0 * cosine) - view;
    let world = Vec3::new(
        reflected.x * optics.rotation.y + reflected.z * optics.rotation.x,
        reflected.y,
        reflected.z * optics.rotation.y - reflected.x * optics.rotation.x,
    );
    // A broad cold sky reflection; only the finite stellar disk is a sharp source.
    let horizon = (world.y * 0.5 + 0.5).clamp(0.0, 1.0);
    let band = (1.0 - (world.dot(&Vec3::new(0.4, 0.8, 0.2))).abs()).powi(3);
    let environment = Vec3::new(12.0, 18.0, 32.0)
        + Vec3::new(36.0, 53.0, 72.0) * horizon
        + Vec3::new(25.0, 14.0, 36.0) * band;
    let toward_center = -p.dot(&light);
    let visibility = if toward_center > 0.0 {
        noise::smoothstep(
            optics.ground_radius.powi(2) * 0.95,
            optics.ground_radius.powi(2) * 1.02,
            p.norm_squared() - toward_center.powi(2),
        )
    } else {
        1.0
    };
    let roughness = if material.kind == Kind::Glass {
        0.004
    } else {
        0.009
    };
    let sun = noise::smoothstep(
        optics.sun_cosine - roughness,
        optics.sun_cosine + 0.003,
        reflected.dot(&light),
    ) * visibility;
    let reflection = (environment + Vec3::new(5200.0, 5800.0, 6500.0) * sun) * f;
    if material.kind == Kind::Glass {
        let path = 0.12 * (material.opacity / 0.36) / (cosine.sqrt() + 0.2);
        let transmission = if k < 0.0 {
            Vec3::zeros()
        } else {
            absorption(material.tint, path) * (1.0 - f)
        };
        let refracted = -view * eta + normal * (eta * cosine - k.max(0.0).sqrt());
        let deviation = refracted + view;
        let refraction = Vec2::new(
            deviation.dot(&optics.screen_right),
            -deviation.dot(&optics.screen_up),
        ) * optics.pixel_scale
            * 0.045
            / cosine.max(0.2);
        return Fragment {
            radiance: reflection,
            transmission,
            refraction,
        };
    }
    let color = if material.kind == Kind::Ground {
        ground_palette(p)
    } else {
        material.tint
    };
    let base = Vec3::new(color.r as f32, color.g as f32, color.b as f32);
    let diffuse = normal.dot(&light).max(0.0) * visibility;
    Fragment {
        radiance: base * (0.32 + diffuse * 0.68) * (1.0 - f * 0.55) + reflection,
        transmission: Vec3::zeros(),
        refraction: Vec2::zeros(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn total_internal_reflection_removes_transmission_without_invalid_values() {
        let optics = Optics {
            eye: Vec3::new(0.8660254, 0.0, 0.5) * 3.0,
            light: Vec3::new(4.0, 6.0, 7.0),
            normal: -Vec3::z(),
            screen_right: Vec3::x(),
            screen_up: Vec3::y(),
            pixel_scale: 100.0,
            ground_radius: 0.7,
            sun_cosine: 0.97,
            rotation: Vec2::new(0.0, 1.0),
        };
        let result = shade(
            Material::new(Color::new(71, 170, 219), 0.34),
            optics,
            Vec3::new(0.0, 0.0, 1.0),
        );
        assert_eq!(result.transmission, Vec3::zeros());
        assert!(
            result
                .radiance
                .iter()
                .chain(result.refraction.iter())
                .all(|v| v.is_finite())
        );
    }

    #[test]
    fn glass_reflects_more_at_grazing_angles_and_absorbs_by_thickness() {
        assert!((fresnel(1.0) - 0.0452).abs() < 0.001);
        assert!(fresnel(0.1) > fresnel(0.8) * 8.0);
        let tint = Color::new(71, 170, 219);
        let thin = absorption(tint, 0.1);
        let thick = absorption(tint, 0.8);
        assert!(thin.iter().zip(thick.iter()).all(|(a, b)| a > b));
        assert!(thick.z > thick.y && thick.y > thick.x);
    }
}
