use crate::{
    framebuffer::Framebuffer,
    noise::{fbm, hash, smoothstep},
    ui::mix,
};
use nalgebra_glm::Vec3;
use rayon::prelude::*;
use std::sync::OnceLock;

struct Sprite {
    width: usize,
    height: usize,
    pixels: Vec<u32>,
}

fn pack(color: Vec3) -> u32 {
    let color = color.map(|channel| channel.clamp(0.0, 255.0).round() as u32);
    color.x << 16 | color.y << 8 | color.z
}

fn generate(style: usize) -> Sprite {
    let (width, height) = (512, 360);
    let mut pixels = vec![0; width * height];
    let (inclination, tilt): (f32, f32) = match style {
        0 => (0.50, -0.36),
        1 => (0.78, 0.20),
        _ => (0.23, -0.17),
    };
    let (sin, cos) = tilt.sin_cos();
    pixels
        .par_chunks_mut(width)
        .enumerate()
        .for_each(|(y, row)| {
            for (x, pixel) in row.iter_mut().enumerate() {
                let px = (x as f32 + 0.5 - width as f32 * 0.5) / 210.0;
                let py = (y as f32 + 0.5 - height as f32 * 0.5) / 210.0;
                let local_x = px * cos + py * sin;
                let local_y = -px * sin + py * cos;
                let q = Vec3::new(local_x, local_y / inclination, style as f32 * 13.7);
                let r = (q.x * q.x + q.y * q.y).sqrt();
                let edge = 1.0 - smoothstep(0.87, 1.13, r);
                if edge == 0.0 {
                    continue;
                }
                let clouds = fbm(q * 6.5 + Vec3::new(7.0, 3.0, 1.0));
                let detail = fbm(q * 26.0 + Vec3::new(12.0, 19.0, 5.0));
                let angle = q.y.atan2(q.x);
                let mut color = match style {
                    0 => {
                        let phase = angle * 3.0 - (r + 0.12).ln() * 8.8 + (clouds - 0.5) * 3.0;
                        let arms = (0.5 + phase.cos() * 0.5).powf(4.0);
                        let dust = (0.5 + (phase + 1.1).cos() * 0.5).powf(7.0) * 0.72;
                        let envelope = (-r * r * 2.3).exp();
                        let core = (-r * r * 96.0).exp();
                        Vec3::new(255.0, 245.0, 227.0) * core * 1.7
                            + Vec3::new(214.0, 155.0, 171.0) * (-r * r * 15.0).exp() * 0.7
                            + (Vec3::new(108.0, 61.0, 192.0)
                                + Vec3::new(122.0, 32.0, 56.0) * (1.0 - r).max(0.0))
                                * (0.16 + arms * (0.6 + detail * 1.5))
                                * envelope
                                * (1.0 - dust)
                    }
                    1 => {
                        let phase = angle * 2.0 - (r + 0.18).ln() * 5.4 + (clouds - 0.5) * 2.6;
                        let arms = (0.5 + phase.cos() * 0.5).powf(5.0);
                        let dust =
                            (0.5 + (phase + 0.7).cos() * 0.5).powf(8.0) * smoothstep(0.08, 0.3, r);
                        let bar = (-(q.x * q.x * 20.0 + q.y * q.y * 94.0)).exp();
                        let bulge = (-r * r * 22.0).exp();
                        (Vec3::new(255.0, 237.0, 189.0) * (bulge * 0.95 + bar * 1.1)
                            + Vec3::new(146.0, 152.0, 173.0) * (-r * r * 4.0).exp() * 0.35
                            + Vec3::new(96.0, 151.0, 225.0)
                                * arms
                                * (-r * r * 1.7).exp()
                                * (0.3 + detail * 1.2))
                            * (1.0 - dust * 0.66)
                    }
                    _ => {
                        let vertical = (-(local_y / 0.11).powi(2)).exp();
                        let disk = (-local_x * local_x * 2.8).exp() * vertical;
                        let bulge = (-(local_x * local_x * 32.0 + local_y * local_y * 65.0)).exp();
                        let lane = (-(local_y / 0.024 + (clouds - 0.5) * 0.7).powi(2)).exp();
                        let blue = Vec3::new(59.0, 139.0, 221.0) * disk * (0.45 + detail);
                        (blue + Vec3::new(243.0, 224.0, 244.0) * bulge * 1.3) * (1.0 - lane * 0.82)
                            + Vec3::new(99.0, 72.0, 170.0) * (-r * r * 3.0).exp() * 0.16
                    }
                };
                // Las regiones de formacion estelar se concentran en el gas exterior al nucleo.
                let grain = hash(x as i32, y as i32, style as i32 * 47 + 19);
                let sparkle = ((grain - 0.975) / 0.025).max(0.0).powi(2)
                    * (0.25 + clouds)
                    * (-r * r * 2.0).exp();
                color += Vec3::new(153.0, 192.0, 255.0) * sparkle;
                color *= edge * (0.7 + detail * 0.6);
                *pixel = pack(color);
            }
        });
    Sprite {
        width,
        height,
        pixels,
    }
}

fn sprites() -> &'static [Sprite; 3] {
    static CACHE: OnceLock<[Sprite; 3]> = OnceLock::new();
    CACHE.get_or_init(|| std::array::from_fn(generate))
}

pub fn prepare() {
    let _ = sprites();
}

/// Sample the cached galaxy illustration for the intergalactic portal's interior.
pub fn sample(style: usize, x: f32, y: f32) -> u32 {
    let sprite = &sprites()[style];
    let x = (x * 0.5 + 0.5) * (sprite.width - 1) as f32;
    let y = (y * 0.5 + 0.5) * (sprite.height - 1) as f32;
    if x < 0.0 || y < 0.0 || x >= (sprite.width - 1) as f32 || y >= (sprite.height - 1) as f32 {
        return 0;
    }
    let (left, top) = (x.floor() as usize, y.floor() as usize);
    let (u, v) = (x - left as f32, y - top as f32);
    mix(
        mix(
            sprite.pixels[top * sprite.width + left],
            sprite.pixels[top * sprite.width + left + 1],
            u,
        ),
        mix(
            sprite.pixels[(top + 1) * sprite.width + left],
            sprite.pixels[(top + 1) * sprite.width + left + 1],
            u,
        ),
        v,
    )
}

pub fn draw(fb: &mut Framebuffer, style: usize, center: (f32, f32), zoom: f32, seconds: f32) {
    let sprite = &sprites()[style];
    let scale = zoom * 0.86;
    let angle = (seconds * 0.025).sin() * 0.09;
    let (sin, cos) = angle.sin_cos();
    let radius_x = sprite.width as f32 * scale * 0.55;
    let radius_y = sprite.height as f32 * scale * 0.6;
    let bounds = (
        (center.0 - radius_x).max(0.0) as usize,
        (center.1 - radius_y).max(0.0) as usize,
        (center.0 + radius_x).ceil().max(0.0).min(fb.width as f32) as usize,
        (center.1 + radius_y).ceil().max(0.0).min(fb.height as f32) as usize,
    );
    fb.shade_rect(bounds, |x, y, pixel, _| {
        let dx = (x as f32 + 0.5 - center.0) / scale;
        let dy = (y as f32 + 0.5 - center.1) / scale;
        let source_x = dx * cos + dy * sin + sprite.width as f32 * 0.5 - 0.5;
        let source_y = -dx * sin + dy * cos + sprite.height as f32 * 0.5 - 0.5;
        if source_x < 0.0
            || source_y < 0.0
            || source_x >= sprite.width as f32 - 1.0
            || source_y >= sprite.height as f32 - 1.0
        {
            return;
        }
        let (left, top) = (source_x.floor() as usize, source_y.floor() as usize);
        let u = source_x - left as f32;
        let v = source_y - top as f32;
        let upper = mix(
            sprite.pixels[top * sprite.width + left],
            sprite.pixels[top * sprite.width + left + 1],
            u,
        );
        let lower = mix(
            sprite.pixels[(top + 1) * sprite.width + left],
            sprite.pixels[(top + 1) * sprite.width + left + 1],
            u,
        );
        let light = mix(upper, lower, v);
        let channel = |shift: u32| (((*pixel >> shift) & 255) + ((light >> shift) & 255)).min(255);
        *pixel = channel(16) << 16 | channel(8) << 8 | channel(0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn galaxies_have_distinct_shapes_bright_cores_and_repeatable_art() {
        let art = sprites();
        let bright = |sprite: &Sprite| {
            sprite
                .pixels
                .iter()
                .filter(|&&pixel| {
                    ((pixel >> 16) & 255)
                        .max((pixel >> 8) & 255)
                        .max(pixel & 255)
                        > 100
                })
                .count()
        };
        for sprite in art {
            assert!(bright(sprite) > 1500);
        }
        assert_ne!(art[0].pixels, art[1].pixels);
        assert_ne!(art[1].pixels, art[2].pixels);
        assert!(bright(&art[2]) < bright(&art[1]));
        assert_eq!(generate(0).pixels, art[0].pixels);
    }
}
