use image::{ImageBuffer, Rgb};
use rayon::prelude::*;

pub const BACKGROUND: u32 = 0x0c121e;

pub(crate) fn emission_color(background: u32, color: u32, strength: f32, shadow: f32) -> u32 {
    let channel = |shift: u32| {
        let from = ((background >> shift) & 255) as f32 * (1.0 - shadow.clamp(0.0, 1.0));
        let light = ((color >> shift) & 255) as f32 * strength.clamp(0.0, 1.0);
        (255.0 - (255.0 - from) * (1.0 - light / 255.0)).round() as u32
    };
    channel(16) << 16 | channel(8) << 8 | channel(0)
}

pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
    pub(crate) depth: Vec<f32>,
    pub(crate) clip: (usize, usize, usize, usize),
}

impl Framebuffer {
    pub fn shade_rect(
        &mut self,
        bounds: (usize, usize, usize, usize),
        shader: impl Fn(usize, usize, &mut u32, &mut f32) + Sync + Send,
    ) {
        let (left, top, right, bottom) = self.clip;
        let x0 = bounds.0.max(left);
        let y0 = bounds.1.max(top);
        let x1 = bounds.2.min(right);
        let y1 = bounds.3.min(bottom);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let width = self.width;
        let pixels = &mut self.pixels[y0 * width..y1 * width];
        let depths = &mut self.depth[y0 * width..y1 * width];
        let row = |(offset, (pixels, depths)): (usize, (&mut [u32], &mut [f32]))| {
            for x in x0..x1 {
                shader(x, y0 + offset, &mut pixels[x], &mut depths[x]);
            }
        };
        // Los fragmentos de filas diferentes se calculan sin compartir escrituras.
        if (x1 - x0) * (y1 - y0) >= 16384 && rayon::current_num_threads() > 1 {
            pixels
                .par_chunks_mut(width)
                .zip(depths.par_chunks_mut(width))
                .enumerate()
                .for_each(row);
        } else {
            pixels
                .chunks_mut(width)
                .zip(depths.chunks_mut(width))
                .enumerate()
                .for_each(row);
        }
    }

    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![BACKGROUND; width * height],
            depth: vec![f32::NEG_INFINITY; width * height],
            clip: (0, 0, width, height),
        }
    }

    pub fn clear(&mut self) {
        self.pixels.fill(BACKGROUND);
        self.depth.fill(f32::NEG_INFINITY);
    }

    pub fn set_clip(&mut self, x: usize, y: usize, width: usize, height: usize) {
        self.clip = (
            x,
            y,
            (x + width).min(self.width),
            (y + height).min(self.height),
        );
    }

    pub fn reset_clip(&mut self) {
        self.clip = (0, 0, self.width, self.height);
    }

    pub fn point(&mut self, x: i32, y: i32, z: f32, color: u32, depth_test: bool) {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        let (left, top, right, bottom) = self.clip;
        if (x as usize) < left || (y as usize) < top || x as usize >= right || y as usize >= bottom
        {
            return;
        }
        let index = y as usize * self.width + x as usize;
        if !depth_test || z > self.depth[index] {
            if depth_test {
                self.depth[index] = z;
            }
            self.pixels[index] = color;
        }
    }

    /// Transparent layers test opaque depth without hiding later transparent layers.
    pub fn blend_depth(&mut self, x: usize, y: usize, depth: f32, color: u32, alpha: f32) {
        let (left, top, right, bottom) = self.clip;
        if x < left || y < top || x >= right || y >= bottom {
            return;
        }
        let index = y * self.width + x;
        if depth <= self.depth[index] {
            return;
        }
        let alpha = alpha.clamp(0.0, 1.0);
        let background = self.pixels[index];
        let channel = |shift: u32| {
            let from = ((background >> shift) & 255) as f32;
            let to = ((color >> shift) & 255) as f32;
            (from + (to - from) * alpha).round() as u32
        };
        self.pixels[index] = channel(16) << 16 | channel(8) << 8 | channel(0);
    }

    #[cfg(test)]
    pub fn emission_depth(
        &mut self,
        point: (usize, usize),
        depth: f32,
        color: u32,
        strength: f32,
        shadow: f32,
    ) {
        let (x, y) = point;
        let (left, top, right, bottom) = self.clip;
        if x < left || y < top || x >= right || y >= bottom {
            return;
        }
        let index = y * self.width + x;
        if depth <= self.depth[index] {
            return;
        }
        self.pixels[index] = emission_color(self.pixels[index], color, strength, shadow);
        if shadow >= 0.999 {
            self.depth[index] = depth;
        }
    }

    pub fn save_png(&self, path: &str) -> Result<(), image::ImageError> {
        let mut image = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(self.width as u32, self.height as u32);
        for (index, pixel) in self.pixels.iter().enumerate() {
            let x = (index % self.width) as u32;
            let y = (index / self.width) as u32;
            image.put_pixel(
                x,
                y,
                Rgb([
                    ((pixel >> 16) & 0xff) as u8,
                    ((pixel >> 8) & 0xff) as u8,
                    (pixel & 0xff) as u8,
                ]),
            );
        }
        image.save(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emission_preserves_foreground_and_black_shadows_occlude_background() {
        let mut fb = Framebuffer::new(3, 1);
        fb.pixels.fill(0xffffff);
        fb.point(0, 0, 0.5, 0x223344, true);
        fb.emission_depth((0, 0), 0.01, 0xffffff, 1.0, 1.0);
        assert_eq!(fb.pixels[0], 0x223344);
        fb.emission_depth((1, 0), 0.01, 0, 1.0, 1.0);
        assert_eq!(fb.pixels[1], 0);
        fb.point(1, 0, 0.001, 0xffffff, true);
        assert_eq!(fb.pixels[1], 0);
        fb.point(1, 0, 0.1, 0xabcdef, true);
        assert_eq!(fb.pixels[1], 0xabcdef);
        fb.emission_depth((2, 0), 0.01, 0xffffff, 0.0, 0.0);
        assert_eq!(fb.pixels[2], 0xffffff);
    }
}
