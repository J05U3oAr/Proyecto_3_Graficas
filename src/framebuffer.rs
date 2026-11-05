use image::{ImageBuffer, Rgb};

pub const BACKGROUND: u32 = 0x0c121e;

pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
    depth: Vec<f32>,
    clip: (usize, usize, usize, usize),
}

impl Framebuffer {
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
