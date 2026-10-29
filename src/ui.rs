use crate::{
    camera::Camera,
    fleet::Ship,
    framebuffer::Framebuffer,
    renderer::{self, RenderMode, Viewport},
    triangle,
};
use image::RgbaImage;
use nalgebra_glm::Vec3;
use std::{collections::HashMap, error::Error, f32::consts::TAU};

pub const WIDTH: usize = 1280;
pub const HEIGHT: usize = 840;
const TEXT: u32 = 0xf2f5fb;
const MUTED: u32 = 0xa0aec2;
const TEAL: u32 = 0x78eed5;
const BORDER: u32 = 0x2a3549;

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}
impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }
    pub fn contains(self, point: (f32, f32)) -> bool {
        point.0 >= self.x as f32
            && point.0 < (self.x + self.w) as f32
            && point.1 >= self.y as f32
            && point.1 < (self.y + self.h) as f32
    }
}

pub const LAUNCH: Rect = Rect::new(1048, 733, 180, 56);
pub const PAUSE: Rect = Rect::new(1030, 142, 198, 40);
pub const BACK: Rect = Rect::new(52, 110, 182, 40);
#[derive(Clone, Copy, Default)]
pub struct MenuView {
    pub selected: usize,
    pub seconds: f32,
    pub paused: bool,
    pub hover: Option<usize>,
    pub hover_button: bool,
    pub hover_pause: bool,
    pub emphasis: [f32; 3],
}
pub const fn card(index: usize) -> Rect {
    Rect::new(52 + index as i32 * 400, 260, 376, 416)
}

/// Mouse positions include letterboxing when the native window is resized.
pub fn canvas_mouse(position: (f32, f32), window: (usize, usize)) -> Option<(f32, f32)> {
    let scale = (window.0 as f32 / WIDTH as f32).min(window.1 as f32 / HEIGHT as f32);
    if scale <= 0.0 {
        return None;
    }
    let offset = (
        (window.0 as f32 - WIDTH as f32 * scale) * 0.5,
        (window.1 as f32 - HEIGHT as f32 * scale) * 0.5,
    );
    let mapped = (
        (position.0 - offset.0) / scale,
        (position.1 - offset.1) / scale,
    );
    Rect::new(0, 0, WIDTH as i32, HEIGHT as i32)
        .contains(mapped)
        .then_some(mapped)
}

pub fn preview_camera(index: usize, seconds: f32) -> Camera {
    Camera {
        yaw: (-0.8 + index as f32 * 0.65 + seconds * TAU / 18.0).rem_euclid(TAU),
        pitch: 0.52 + (seconds * TAU / 18.0).sin() * 0.055,
        zoom: 1.08,
    }
}

#[derive(Clone)]
struct Glyph {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    left: i32,
    top: i32,
    advance: f32,
}
struct Fonts {
    atlas: RgbaImage,
    glyphs: HashMap<(u32, char), Glyph>,
}
impl Fonts {
    fn load() -> Result<Self, Box<dyn Error>> {
        let atlas =
            image::load_from_memory(include_bytes!("../assets/ui/type-atlas.png"))?.to_rgba8();
        let mut glyphs = HashMap::new();
        for line in include_str!("../assets/ui/type-atlas.txt").lines() {
            let p: Vec<&str> = line.split_whitespace().collect();
            let size: u32 = p[0].parse()?;
            let character = char::from_u32(p[1].parse()?).ok_or("Invalid atlas character")?;
            glyphs.insert(
                (size, character),
                Glyph {
                    x: p[2].parse()?,
                    y: p[3].parse()?,
                    w: p[4].parse()?,
                    h: p[5].parse()?,
                    left: p[6].parse()?,
                    top: p[7].parse()?,
                    advance: p[8].parse()?,
                },
            );
        }
        Ok(Self { atlas, glyphs })
    }
    fn glyph(&self, size: u32, c: char) -> &Glyph {
        self.glyphs
            .get(&(size, c))
            .unwrap_or_else(|| &self.glyphs[&(size, '?')])
    }
    fn width(&self, size: u32, text: &str) -> f32 {
        text.chars().map(|c| self.glyph(size, c).advance).sum()
    }
    fn text(&self, fb: &mut Framebuffer, size: u32, x: i32, y: i32, text: &str, color: u32) {
        let mut cursor = x as f32;
        for c in text.chars() {
            let glyph = self.glyph(size, c);
            for row in 0..glyph.h {
                for col in 0..glyph.w {
                    let alpha = self.atlas.get_pixel(glyph.x + col, glyph.y + row)[3];
                    if alpha != 0 {
                        blend_pixel(
                            fb,
                            cursor.round() as i32 + glyph.left + col as i32,
                            y + size as i32 + glyph.top + row as i32,
                            color,
                            alpha as f32 / 255.0,
                        );
                    }
                }
            }
            cursor += glyph.advance;
        }
    }
}

pub struct Ui {
    fonts: Fonts,
    background: Vec<u32>,
}
impl Ui {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let mut background = Framebuffer::new(WIDTH, HEIGHT);
        for y in 0..HEIGHT {
            let t = y as f32 / HEIGHT as f32;
            let color = mix(0x101827, 0x070c16, t);
            background.pixels[y * WIDTH..(y + 1) * WIDTH].fill(color);
        }
        glow(
            &mut background,
            (1030.0, 45.0),
            (660.0, 340.0),
            0x244468,
            0.36,
        );
        glow(
            &mut background,
            (90.0, 750.0),
            (630.0, 350.0),
            0x193c3a,
            0.2,
        );
        let mut seed = 13875u32;
        for i in 0..110 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let x = (seed % WIDTH as u32) as i32;
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let y = (seed % HEIGHT as u32) as i32;
            blend_pixel(
                &mut background,
                x,
                y,
                0xadc6e2,
                if i % 8 == 0 { 0.65 } else { 0.22 },
            );
            if i % 16 == 0 {
                rule(&mut background, (x - 2, y), (x + 2, y), 0x52677e);
                rule(&mut background, (x, y - 2), (x, y + 2), 0x52677e);
            }
        }
        Ok(Self {
            fonts: Fonts::load()?,
            background: background.pixels,
        })
    }

    fn begin(&self, fb: &mut Framebuffer) {
        fb.clear();
        fb.pixels.copy_from_slice(&self.background);
        self.header(fb);
    }

    fn header(&self, fb: &mut Framebuffer) {
        ellipse(fb, (68.0, 47.0), (17.0, 9.0), TEAL);
        ellipse(fb, (68.0, 47.0), (9.0, 17.0), 0x4a8589);
        rounded(fb, Rect::new(65, 44, 6, 6), 3, TEAL, TEAL);
        self.fonts.text(fb, 22, 102, 30, "ORBITAL", TEXT);
        rule(fb, (52, 85), (1228, 85), BORDER);
    }

    pub fn menu(&self, fb: &mut Framebuffer, ships: &[Ship], view: &MenuView) {
        let MenuView {
            selected,
            seconds,
            paused,
            hover,
            hover_button,
            hover_pause,
            emphasis,
        } = *view;
        self.begin(fb);
        self.fonts
            .text(fb, 52, 50, 120, "Elige tu próxima nave.", TEXT);
        rounded(
            fb,
            PAUSE,
            9,
            if hover_pause { 0x25364a } else { 0x172233 },
            BORDER,
        );
        if paused {
            arrow(fb, 1048, 155, 0xb9c8dc);
        } else {
            rounded(fb, Rect::new(1048, 155, 3, 13), 1, MUTED, MUTED);
            rounded(fb, Rect::new(1055, 155, 3, 13), 1, MUTED, MUTED);
        }
        self.fonts.text(
            fb,
            16,
            1072,
            151,
            if paused {
                "Reanudar giro"
            } else {
                "Pausar giro"
            },
            TEXT,
        );
        for (index, ship) in ships.iter().enumerate() {
            let r = card(index);
            let active = index == selected;
            let amount = emphasis[index];
            let edge = mix(BORDER, ship.accent, amount * 0.85);
            rounded(
                fb,
                Rect::new(r.x, r.y + 8, r.w, r.h),
                18,
                0x060b13,
                0x060b13,
            );
            rounded(fb, r, 18, mix(0x111b2b, 0x1b2b3d, amount * 0.5), edge);
            rule(
                fb,
                (r.x + 22, r.y + 1),
                (r.x + r.w - 22, r.y + 1),
                mix(edge, ship.accent, amount),
            );
            self.fonts.text(
                fb,
                14,
                r.x + 24,
                r.y + 22,
                &format!("0{}", index + 1),
                MUTED,
            );
            self.fonts.text(
                fb,
                14,
                r.x + r.w - 60,
                r.y + 24,
                "360°",
                if active { ship.accent } else { MUTED },
            );

            let platform_y = 518.0;
            let center_x = r.x as f32 + r.w as f32 * 0.5;
            glow(
                fb,
                (center_x, platform_y - 45.0),
                (163.0, 115.0),
                ship.accent,
                0.08 + amount * 0.035,
            );
            glow(
                fb,
                (center_x, platform_y + 3.0),
                (142.0, 27.0),
                ship.accent,
                0.16,
            );
            ellipse(
                fb,
                (center_x, platform_y),
                (136.0, 23.0),
                mix(0x172638, ship.accent, 0.32),
            );
            ellipse(
                fb,
                (center_x, platform_y),
                (106.0, 17.0),
                mix(0x172638, ship.accent, 0.21),
            );
            rule(fb, (r.x + 30, 518), (r.x + 45, 518), edge);
            rule(fb, (r.x + r.w - 45, 518), (r.x + r.w - 30, 518), edge);
            let orbit = seconds * TAU / 18.0 + index as f32 * 0.8;
            let marker = (
                center_x + orbit.cos() * 136.0,
                platform_y + orbit.sin() * 23.0,
            );
            rounded(
                fb,
                Rect::new(marker.0 as i32 - 2, marker.1 as i32 - 2, 5, 5),
                2,
                ship.accent,
                ship.accent,
            );

            renderer::draw_model(
                fb,
                &ship.model,
                &preview_camera(index, seconds),
                RenderMode::Solid,
                Viewport {
                    x: (r.x + 16) as usize,
                    y: 306,
                    width: (r.w - 32) as usize,
                    height: 240,
                },
                ship.finish,
            );
            self.fonts.text(fb, 28, r.x + 24, 566, ship.name, TEXT);
            if hover == Some(index) && !active {
                rounded(
                    fb,
                    Rect::new(r.x + 342, 576, 8, 8),
                    4,
                    ship.accent,
                    ship.accent,
                );
            }
        }

        rule(fb, (52, 711), (1228, 711), BORDER);
        self.fonts.text(fb, 14, 54, 744, "SELECCIONADA", MUTED);
        self.fonts.text(fb, 22, 54, 762, ships[selected].name, TEXT);
        let cta = if hover_button { 0xadffe9 } else { TEAL };
        rounded(fb, LAUNCH, 12, cta, cta);
        let label = "ABRIR NAVE";
        let label_x = LAUNCH.x + (LAUNCH.w - self.fonts.width(18, label).round() as i32 - 20) / 2;
        self.fonts.text(fb, 18, label_x, 749, label, 0x0c2829);
        arrow(fb, LAUNCH.x + LAUNCH.w - 28, 755, 0x0c2829);
        self.fonts.text(
            fb,
            14,
            54,
            805,
            "1–3 Elegir  ·  TAB Recorrer  ·  ENTER Abrir  ·  ESPACIO Pausar  ·  ESC Salir",
            MUTED,
        );
    }

    pub fn inspector(&self, fb: &mut Framebuffer, ship: &Ship, mode: RenderMode, hover_back: bool) {
        self.begin(fb);
        rounded(
            fb,
            BACK,
            9,
            if hover_back { 0x2a3a4c } else { 0x172233 },
            BORDER,
        );
        rule(fb, (67, 131), (80, 131), TEXT);
        rule(fb, (67, 131), (72, 126), TEXT);
        rule(fb, (67, 131), (72, 136), TEXT);
        self.fonts.text(fb, 16, 89, 119, "Volver al hangar", TEXT);
        self.fonts.text(fb, 52, 50, 162, ship.name, TEXT);
        glow(fb, (478.0, 602.0), (380.0, 180.0), ship.accent, 0.13);
        ellipse(
            fb,
            (478.0, 663.0),
            (310.0, 42.0),
            mix(0x162538, ship.accent, 0.25),
        );
        ellipse(
            fb,
            (478.0, 663.0),
            (230.0, 31.0),
            mix(0x162538, ship.accent, 0.15),
        );
        renderer::draw_model(
            fb,
            &ship.model,
            &ship.camera,
            mode,
            Viewport {
                x: 52,
                y: 263,
                width: 850,
                height: 446,
            },
            ship.finish,
        );
        rounded(fb, Rect::new(944, 272, 284, 393), 16, 0x121e2e, BORDER);
        self.fonts.text(fb, 14, 970, 302, "CONTROLES", ship.accent);
        self.fonts
            .text(fb, 16, 970, 332, "Flechas / WASD   Girar", MUTED);
        self.fonts.text(fb, 16, 970, 360, "Q / E   Zoom", MUTED);
        self.fonts
            .text(fb, 16, 970, 388, "M   Cambiar vista", MUTED);
        self.fonts.text(fb, 16, 970, 416, "R   Reiniciar", MUTED);
        rule(fb, (970, 456), (1202, 456), BORDER);
        self.fonts.text(fb, 14, 970, 480, "VISTA", MUTED);
        self.fonts.text(fb, 28, 970, 504, mode.label(), TEXT);
        rule(fb, (52, 747), (1228, 747), BORDER);
        self.fonts.text(
            fb,
            16,
            54,
            774,
            "1–3  Cambiar nave     P  Guardar captura",
            MUTED,
        );
        self.fonts
            .text(fb, 16, 1000, 774, "ESC  Volver al hangar", MUTED);
    }
}

fn mix(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let channel = |shift: u32| {
        let a = ((a >> shift) & 255) as f32;
        let b = ((b >> shift) & 255) as f32;
        (a + (b - a) * t).round() as u32
    };
    channel(16) << 16 | channel(8) << 8 | channel(0)
}
fn blend_pixel(fb: &mut Framebuffer, x: i32, y: i32, color: u32, alpha: f32) {
    if x >= 0 && y >= 0 && (x as usize) < fb.width && (y as usize) < fb.height {
        let i = y as usize * fb.width + x as usize;
        fb.pixels[i] = mix(fb.pixels[i], color, alpha);
    }
}
fn rounded(fb: &mut Framebuffer, r: Rect, radius: i32, fill: u32, border: u32) {
    rounded_fill(fb, r, radius, border);
    if fill != border {
        rounded_fill(
            fb,
            Rect::new(r.x + 1, r.y + 1, r.w - 2, r.h - 2),
            (radius - 1).max(0),
            fill,
        );
    }
}

// Fill scanlines in bulk; only the two edge pixels need alpha blending.
fn rounded_fill(fb: &mut Framebuffer, r: Rect, radius: i32, color: u32) {
    let radius = radius.max(0).min(r.w / 2).min(r.h / 2) as f32;
    for y in r.y.max(0)..(r.y + r.h).min(fb.height as i32) {
        let dy = ((y as f32 + 0.5 - r.y as f32 - r.h as f32 * 0.5).abs()
            - (r.h as f32 * 0.5 - radius))
            .max(0.0);
        let inset = radius - (radius * radius - dy * dy).max(0.0).sqrt();
        let left = r.x as f32 + inset;
        let right = (r.x + r.w) as f32 - inset;
        let start = (left.ceil() as i32).clamp(0, fb.width as i32) as usize;
        let end = (right.floor() as i32).clamp(0, fb.width as i32) as usize;
        if end > start {
            fb.pixels[y as usize * fb.width + start..y as usize * fb.width + end].fill(color);
        }
        blend_pixel(fb, left.ceil() as i32 - 1, y, color, left.ceil() - left);
        blend_pixel(fb, right.floor() as i32, y, color, right - right.floor());
    }
}
fn glow(fb: &mut Framebuffer, center: (f32, f32), radius: (f32, f32), color: u32, strength: f32) {
    let left = (center.0 - radius.0).max(0.0) as i32;
    let right = (center.0 + radius.0).min(fb.width as f32) as i32;
    let top = (center.1 - radius.1).max(0.0) as i32;
    let bottom = (center.1 + radius.1).min(fb.height as f32) as i32;
    for y in top..bottom {
        for x in left..right {
            let dx = (x as f32 - center.0) / radius.0;
            let dy = (y as f32 - center.1) / radius.1;
            let a = (1.0 - dx * dx - dy * dy).max(0.0);
            if a > 0.0 {
                blend_pixel(fb, x, y, color, a * a * strength);
            }
        }
    }
}
fn rule(fb: &mut Framebuffer, from: (i32, i32), to: (i32, i32), color: u32) {
    triangle::line(
        fb,
        &Vec3::new(from.0 as f32, from.1 as f32, 0.0),
        &Vec3::new(to.0 as f32, to.1 as f32, 0.0),
        color,
        false,
    );
}
fn ellipse(fb: &mut Framebuffer, center: (f32, f32), radius: (f32, f32), color: u32) {
    let mut previous = (center.0 + radius.0, center.1);
    for step in 1..=160 {
        let angle = step as f32 * TAU / 160.0;
        let point = (
            center.0 + angle.cos() * radius.0,
            center.1 + angle.sin() * radius.1,
        );
        rule(
            fb,
            (previous.0.round() as i32, previous.1.round() as i32),
            (point.0.round() as i32, point.1.round() as i32),
            color,
        );
        previous = point;
    }
}
fn arrow(fb: &mut Framebuffer, x: i32, y: i32, color: u32) {
    rule(fb, (x, y + 5), (x + 17, y + 5), color);
    rule(fb, (x + 12, y), (x + 17, y + 5), color);
    rule(fb, (x + 12, y + 10), (x + 17, y + 5), color);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mouse_mapping_respects_letterboxing_and_card_gaps() {
        assert!(canvas_mouse((10.0, 10.0), (1600, 840)).is_none());
        let point = canvas_mouse((260.0, 320.0), (1600, 840)).unwrap();
        assert!(card(0).contains(point));
        assert!(!card(0).contains((440.0, 400.0)));
        assert!(!card(1).contains((440.0, 400.0)));
    }
    #[test]
    fn every_preview_completes_a_revolution_in_eighteen_seconds() {
        for i in 0..3 {
            let before = preview_camera(i, 0.0);
            let after = preview_camera(i, 18.0);
            assert!((before.yaw.cos() - after.yaw.cos()).abs() < 1e-5);
            assert!((before.yaw.sin() - after.yaw.sin()).abs() < 1e-5);
            assert!((before.pitch - after.pitch).abs() < 1e-5);
            assert!((before.yaw.cos() - preview_camera(i, 9.0).yaw.cos()).abs() > 0.1);
        }
    }
    #[test]
    fn all_three_models_animate_inside_their_preview_without_changing_the_menu() {
        let ships = crate::fleet::load().unwrap();
        let ui = Ui::new().unwrap();
        let mut before = Framebuffer::new(WIDTH, HEIGHT);
        let mut after = Framebuffer::new(WIDTH, HEIGHT);
        let view = MenuView {
            emphasis: [1.0, 0.0, 0.0],
            ..Default::default()
        };
        ui.menu(&mut before, &ships, &view);
        ui.menu(
            &mut after,
            &ships,
            &MenuView {
                seconds: 4.0,
                ..view
            },
        );
        let mut changed = [0; 3];
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                if before.pixels[y * WIDTH + x] != after.pixels[y * WIDTH + x] {
                    let index = (0..3)
                        .find(|&i| {
                            Rect::new(card(i).x + 16, 306, 344, 240).contains((x as f32, y as f32))
                        })
                        .expect("Preview geometry escaped its viewport");
                    changed[index] += 1;
                }
            }
        }
        assert!(
            changed.iter().all(|&count| count > 500),
            "A ship preview was empty or static: {changed:?}"
        );
        for size in [14, 16, 18, 22, 28, 52] {
            for character in "áéíóúÓÍÑ°–".chars() {
                assert!(
                    ui.fonts.glyphs.contains_key(&(size, character)),
                    "Missing UI character: {character}"
                );
            }
        }
    }
}
