use crate::color::Color;
use image::RgbImage;
use nalgebra_glm::Vec2;
use std::path::Path;

pub struct Texture {
    pixels: RgbImage,
}

impl Texture {
    pub fn aspect_ratio(&self) -> f32 {
        self.pixels.width() as f32 / self.pixels.height() as f32
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, image::ImageError> {
        Ok(Self {
            pixels: image::open(path)?.to_rgb8(),
        })
    }

    /// OBJ UVs have their origin at the bottom left; image rows start at the top.
    pub fn sample(&self, uv: Vec2) -> Color {
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
        let texture = Texture { pixels };
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
}
