#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn to_hex(self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | self.b as u32
    }

    pub fn shaded(self, intensity: f32) -> Self {
        let scale = |channel: u8| (channel as f32 * intensity).clamp(0.0, 255.0) as u8;
        Self::new(scale(self.r), scale(self.g), scale(self.b))
    }
}
