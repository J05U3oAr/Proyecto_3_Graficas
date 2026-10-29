use image::{ImageBuffer, Rgb};

pub const BACKGROUND: u32 = 0x0c121e;

pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
    depth: Vec<f32>,
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![BACKGROUND; width * height],
            depth: vec![f32::NEG_INFINITY; width * height],
        }
    }

    pub fn clear(&mut self) {
        self.pixels.fill(BACKGROUND);
        self.depth.fill(f32::NEG_INFINITY);
    }

    pub fn point(&mut self, x: i32, y: i32, z: f32, color: u32, depth_test: bool) {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
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
