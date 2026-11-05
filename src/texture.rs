use crate::color::Color;
use image::RgbImage;
use nalgebra_glm::Vec2;
use std::path::Path;

pub struct Texture {
    pixels: RgbImage,
    horizontal_blend: f32,
}

impl Texture {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, image::ImageError> {
        Ok(Self {
            pixels: image::open(path)?.to_rgb8(),
            horizontal_blend: 0.0,
        })
    }

    /// Blend both atlas ends in a narrow UV strip on a spherical longitude seam.
    pub fn blend_longitude_seam(&mut self, width: f32) {
        self.horizontal_blend = width.clamp(0.0, 0.25);
    }

    /// OBJ UVs have their origin at the bottom left; image rows start at the top.
    pub fn sample(&self, uv: Vec2) -> Color {
        let color = self.sample_clamped(uv);
        let edge = uv.x.clamp(0.0, 1.0).min(1.0 - uv.x.clamp(0.0, 1.0));
        if self.horizontal_blend == 0.0 || edge >= self.horizontal_blend {
            return color;
        }
        let t = 1.0 - edge / self.horizontal_blend;
        let weight = 0.5 * t * t * (3.0 - 2.0 * t);
        let opposite = self.sample_clamped(Vec2::new(1.0 - uv.x, uv.y));
        let blend = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * weight).round() as u8;
        Color::new(
            blend(color.r, opposite.r),
            blend(color.g, opposite.g),
            blend(color.b, opposite.b),
        )
    }

    fn sample_clamped(&self, uv: Vec2) -> Color {
        let x = uv.x.clamp(0.0, 1.0) * (self.pixels.width() - 1) as f32;
        let y = (1.0 - uv.y.clamp(0.0, 1.0)) * (self.pixels.height() - 1) as f32;
        let (left, top) = (x.floor() as u32, y.floor() as u32);
        let right = (left + 1).min(self.pixels.width() - 1);
        let bottom = (top + 1).min(self.pixels.height() - 1);
        let (u, v) = (x - left as f32, y - top as f32);
        let channel = |index| {
            let a = self.pixels.get_pixel(left, top)[index] as f32;
            let b = self.pixels.get_pixel(right, top)[index] as f32;
            let c = self.pixels.get_pixel(left, bottom)[index] as f32;
            let d = self.pixels.get_pixel(right, bottom)[index] as f32;
            ((a + (b - a) * u) * (1.0 - v) + (c + (d - c) * u) * v).round() as u8
        };
        Color::new(channel(0), channel(1), channel(2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_orientation_bilinear_filtering_and_edge_clamping_follow_obj_uvs() {
        let pixels =
            RgbImage::from_raw(2, 2, vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]).unwrap();
        let texture = Texture {
            pixels,
            horizontal_blend: 0.0,
        };
        assert_eq!(texture.sample(Vec2::new(0.0, 1.0)), Color::new(255, 0, 0));
        assert_eq!(texture.sample(Vec2::new(0.0, 0.0)), Color::new(0, 0, 255));
        assert_eq!(
            texture.sample(Vec2::new(0.5, 0.5)),
            Color::new(128, 128, 128)
        );
        assert_eq!(
            texture.sample(Vec2::new(3.0, -2.0)),
            Color::new(255, 255, 255)
        );
    }

    #[test]
    fn longitude_blending_joins_atlas_ends_without_changing_the_interior() {
        let mut texture = Texture {
            pixels: RgbImage::from_raw(2, 1, vec![255, 0, 0, 0, 0, 255]).unwrap(),
            horizontal_blend: 0.0,
        };
        let interior = texture.sample(Vec2::new(0.3, 0.5));
        texture.blend_longitude_seam(0.035);
        assert_eq!(
            texture.sample(Vec2::new(0.0, 0.5)),
            texture.sample(Vec2::new(1.0, 0.5))
        );
        assert_eq!(texture.sample(Vec2::new(0.3, 0.5)), interior);
        let a = texture.sample(Vec2::new(0.0001, 0.5));
        let b = texture.sample(Vec2::new(0.9999, 0.5));
        assert!((a.r as i32 - b.r as i32).abs() <= 1);
    }
}
