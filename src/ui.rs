use crate::{
    camera::Camera,
    fleet::Ship,
    flight::Flight,
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
    Rect::new(42, 258 + index as i32 * 130, 250, 112)
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
pub(crate) struct Fonts {
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
    pub(crate) fn width(&self, size: u32, text: &str) -> f32 {
        text.chars().map(|c| self.glyph(size, c).advance).sum()
    }
    pub(crate) fn text(
        &self,
        fb: &mut Framebuffer,
        size: u32,
        x: i32,
        y: i32,
        text: &str,
        color: u32,
    ) {
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
    pub(crate) fonts: Fonts,
    background: Vec<u32>,
    stars: Vec<Star>,
    space: crate::space::Space,
    deep_space: crate::deep_space::DeepSpace,
    galaxy: usize,
}

struct Star {
    position: Vec3,
    phase: f32,
    color: u32,
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
        let mut stars = Vec::with_capacity(900);
        for index in 0..900 {
            let mut random = (index as u32 + 71).wrapping_mul(747_796_405);
            let mut next = || {
                random = random
                    .wrapping_mul(2_891_336_453)
                    .wrapping_add(3_037_000_493);
                let bits = ((random >> ((random >> 28) + 4)) ^ random).wrapping_mul(277_803_737);
                let bits = (bits >> 22) ^ bits;
                bits as f32 / u32::MAX as f32
            };
            stars.push(Star {
                position: Vec3::new(
                    (next() * 2.0 - 1.0) * 280.0,
                    (next() * 2.0 - 1.0) * 280.0,
                    (next() * 2.0 - 1.0) * 280.0,
                ),
                phase: next() * TAU,
                color: [0x8db8e8, 0xc2d7f0, 0xf1e8d8][index % 3],
            });
        }
        Ok(Self {
            galaxy: 0,
            fonts: Fonts::load()?,
            background: background.pixels,
            stars,
            space: crate::space::Space::load()?,
            deep_space: crate::deep_space::DeepSpace::new(),
        })
    }

    pub(crate) fn begin(&self, fb: &mut Framebuffer) {
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
        crate::menus::ships(self, fb, ships, view);
    }

    pub fn galaxy_menu(&self, fb: &mut Framebuffer, ship: &Ship, view: &crate::galaxy::GalaxyView) {
        crate::menus::galaxies(self, fb, ship, view);
    }

    pub fn select_galaxy(&mut self, index: usize) {
        if self.galaxy == index {
            return;
        }
        self.space.configure(index);
        self.galaxy = index;
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
        self.fonts.text(fb, 16, 89, 119, "Volver al vuelo", TEXT);
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
            .text(fb, 16, 1000, 774, "ESC  Volver al vuelo", MUTED);
    }
    pub fn landmark_preview(&self, fb: &mut Framebuffer, id: &str) -> Result<(), Box<dyn Error>> {
        self.deep_space.preview(fb, id)
    }

    pub fn new_flight(&self) -> Flight {
        self.space.new_flight()
    }

    pub fn approach_planet(&self, flight: &mut Flight, name: &str) -> Result<(), Box<dyn Error>> {
        self.space.approach(flight, name)
    }

    pub fn planet_preview(
        &self,
        fb: &mut Framebuffer,
        name: &str,
        seconds: f32,
    ) -> Result<(), Box<dyn Error>> {
        self.space.preview(fb, name, seconds)
    }

    pub fn flight(&self, fb: &mut Framebuffer, ship: &Ship, flight: &Flight, seconds: f32) {
        fb.clear();
        for y in 0..HEIGHT {
            let t = y as f32 / HEIGHT as f32;
            let color = mix(crate::galaxy::GALAXIES[self.galaxy].sky, 0x03060c, t);
            fb.pixels[y * WIDTH..(y + 1) * WIDTH].fill(color);
        }

        let (right, up, forward) = flight.camera.basis();
        let focal = renderer::FLIGHT_FOCAL;
        let eye = flight.camera_position();
        let speed = flight.velocity.norm();
        for star in &self.stars {
            // Recycle a world-space volume around the camera for endless travel.
            let relative = (star.position - eye).map(|v| (v + 280.0).rem_euclid(560.0) - 280.0);
            let depth = relative.dot(&forward);
            if !(3.0..280.0).contains(&depth) {
                continue;
            }
            let x = WIDTH as f32 * 0.5 + relative.dot(&right) * focal / depth;
            let y = HEIGHT as f32 * 0.5 - relative.dot(&up) * focal / depth;
            if x < 1.0 || y < 1.0 || x >= WIDTH as f32 - 1.0 || y >= HEIGHT as f32 - 1.0 {
                continue;
            }
            let twinkle = 0.65 + ((seconds * 1.7 + star.phase).sin() + 1.0) * 0.16;
            let color = mix(0x26364a, star.color, twinkle);
            let point = (x.round() as i32, y.round() as i32);
            if speed > 14.0 {
                let forward_speed = flight.velocity.dot(&forward);
                let vx = (relative.dot(&right) * forward_speed / depth
                    - flight.velocity.dot(&right))
                    * focal
                    / depth
                    * 0.045;
                let vy = (-relative.dot(&up) * forward_speed / depth + flight.velocity.dot(&up))
                    * focal
                    / depth
                    * 0.045;
                let vx = vx.clamp(-24.0, 24.0);
                let vy = vy.clamp(-24.0, 24.0);
                rule(
                    fb,
                    (point.0, point.1),
                    ((x - vx).round() as i32, (y - vy).round() as i32),
                    color,
                );
            } else {
                blend_pixel(fb, point.0, point.1, color, 0.8);
            }
        }

        self.space.draw(fb, flight, seconds);
        if flight.crash.is_none() {
            renderer::draw_flight_model(fb, &ship.model, flight, ship.finish);
        }
        self.deep_space.draw(fb, flight);
        self.space.draw_atmospheres(fb, flight, seconds);
        if let Some(crash) = &flight.crash {
            crate::explosion::draw(fb, &renderer::FlightView::new(flight, WIDTH, HEIGHT), crash);
        }

        if flight.is_captured() {
            let fade = 0.35 + flight.capture_progress() * 0.65;
            for pixel in &mut fb.pixels {
                *pixel = mix(*pixel, 0x000000, fade);
            }
        }

        let view = renderer::FlightView::new(flight, WIDTH, HEIGHT);
        let hole = crate::black_hole::BLACK_HOLE;
        let distance = (flight.position - hole.position).norm();
        if let Some(point) = view.project(hole.position) {
            let y = point.y + hole.horizon * renderer::FLIGHT_FOCAL * point.z + 22.0;
            let label = format!("Agujero negro  {:.0} u", distance);
            if (180.0..WIDTH as f32 - 180.0).contains(&point.x) && (145.0..650.0).contains(&y) {
                self.fonts.text(
                    fb,
                    14,
                    (point.x - self.fonts.width(14, &label) * 0.5) as i32,
                    y as i32,
                    &label,
                    0xffba69,
                );
            }
        }
        for planet in self.space.active_planets() {
            if let Some(point) = view.project(planet.transform.position) {
                let atmosphere = if planet.dust.is_some() { 1.22 } else { 1.0 };
                let y = point.y
                    + planet.transform.radius * atmosphere * renderer::FLIGHT_FOCAL * point.z
                    + 14.0;
                if (80.0..WIDTH as f32 - 80.0).contains(&point.x) && (145.0..730.0).contains(&y) {
                    let x = point.x - self.fonts.width(14, planet.name) * 0.5;
                    self.fonts
                        .text(fb, 14, x as i32, y as i32, planet.name, MUTED);
                }
            }
        }

        rounded(fb, Rect::new(34, 25, 285, 66), 12, 0x101d2c, 0x304154);
        self.fonts.text(
            fb,
            14,
            54,
            37,
            crate::galaxy::GALAXIES[self.galaxy].name,
            TEAL,
        );
        self.fonts.text(fb, 22, 54, 56, ship.name, TEXT);
        rounded(fb, Rect::new(970, 25, 276, 92), 12, 0x101d2c, 0x304154);
        self.fonts.text(
            fb,
            14,
            992,
            38,
            &format!("VELOCIDAD  {:05.1}", speed),
            ship.accent,
        );
        self.fonts.text(
            fb,
            14,
            992,
            67,
            &format!(
                "X {:>6.0}  Y {:>6.0}  Z {:>6.0}",
                flight.position.x, flight.position.y, flight.position.z
            ),
            TEXT,
        );

        let center = (WIDTH as i32 / 2, HEIGHT as i32 / 2);
        rule(
            fb,
            (center.0 - 15, center.1),
            (center.0 - 6, center.1),
            0x7892a8,
        );
        rule(
            fb,
            (center.0 + 6, center.1),
            (center.0 + 15, center.1),
            0x7892a8,
        );
        rule(
            fb,
            (center.0, center.1 - 15),
            (center.0, center.1 - 6),
            0x7892a8,
        );
        rule(
            fb,
            (center.0, center.1 + 6),
            (center.0, center.1 + 15),
            0x7892a8,
        );

        let crash_detail = flight
            .crash
            .as_ref()
            .map(|crash| format!("Impacto contra {}. Regresando al inicio...", crash.planet));
        let warning = if let Some(detail) = crash_detail.as_deref() {
            Some(("NAVE DESTRUIDA", detail))
        } else if flight.is_captured() {
            Some(("NAVE ABSORBIDA", "Regresando al punto de partida..."))
        } else if flight.respawn_notice > 0.0 {
            Some(("NAVE RECUPERADA", "Has regresado al punto de partida."))
        } else if flight.contact_notice > 0.0 {
            Some((
                "CONTACTO CON LA SUPERFICIE",
                "Retrocede o gira para separarte del planeta.",
            ))
        } else if hole.strength(flight.position) > 0.001 {
            Some((
                "ATRACCION GRAVITACIONAL",
                "Alejate con WASD + SHIFT para intentar escapar.",
            ))
        } else {
            None
        };
        if let Some((title, detail)) = warning {
            rounded(fb, Rect::new(385, 680, 510, 66), 12, 0x241a16, 0xbe703c);
            self.fonts.text(fb, 16, 405, 690, title, 0xffba69);
            self.fonts.text(fb, 14, 405, 718, detail, TEXT);
        }
        rounded(fb, Rect::new(34, 770, 1212, 43), 10, 0x101d2c, 0x304154);
        self.fonts.text(
            fb,
            14,
            52,
            784,
            "W/S Avanzar/Retroceder  ·  A/D Lateral  ·  Mouse Dirigir  ·  SHIFT Turbo  ·  ESPACIO Freno  ·  I Vista  ·  ESC Galaxias",
            MUTED,
        );
    }
}

pub(crate) fn mix(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let channel = |shift: u32| {
        let a = ((a >> shift) & 255) as f32;
        let b = ((b >> shift) & 255) as f32;
        (a + (b - a) * t).round() as u32
    };
    channel(16) << 16 | channel(8) << 8 | channel(0)
}
pub(crate) fn blend_pixel(fb: &mut Framebuffer, x: i32, y: i32, color: u32, alpha: f32) {
    if x >= 0 && y >= 0 && (x as usize) < fb.width && (y as usize) < fb.height {
        let i = y as usize * fb.width + x as usize;
        fb.pixels[i] = mix(fb.pixels[i], color, alpha);
    }
}
pub(crate) fn rounded(fb: &mut Framebuffer, r: Rect, radius: i32, fill: u32, border: u32) {
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
pub(crate) fn rounded_fill(fb: &mut Framebuffer, r: Rect, radius: i32, color: u32) {
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
pub(crate) fn glow(
    fb: &mut Framebuffer,
    center: (f32, f32),
    radius: (f32, f32),
    color: u32,
    strength: f32,
) {
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
pub(crate) fn rule(fb: &mut Framebuffer, from: (i32, i32), to: (i32, i32), color: u32) {
    triangle::line(
        fb,
        &Vec3::new(from.0 as f32, from.1 as f32, 0.0),
        &Vec3::new(to.0 as f32, to.1 as f32, 0.0),
        color,
        false,
    );
}
pub(crate) fn ellipse(fb: &mut Framebuffer, center: (f32, f32), radius: (f32, f32), color: u32) {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moving_through_space_changes_the_visible_star_field() {
        let ships = crate::fleet::load().unwrap();
        let ui = Ui::new().unwrap();
        let mut flight = Flight::new();
        let mut before = Framebuffer::new(WIDTH, HEIGHT);
        let mut after = Framebuffer::new(WIDTH, HEIGHT);
        ui.flight(&mut before, &ships[0], &flight, 0.0);
        flight.position += flight.camera.basis().2 * 40.0;
        ui.flight(&mut after, &ships[0], &flight, 0.0);
        let changed = (140..740)
            .flat_map(|y| (0..WIDTH).map(move |x| y * WIDTH + x))
            .filter(|&index| before.pixels[index] != after.pixels[index])
            .count();
        assert!(
            changed > 100,
            "Forward flight must move the stars: {changed}"
        );
    }
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
    fn each_selected_ship_animates_without_changing_the_sidebar_or_details() {
        let ships = crate::fleet::load().unwrap();
        let ui = Ui::new().unwrap();
        let mut before = Framebuffer::new(WIDTH, HEIGHT);
        let mut after = Framebuffer::new(WIDTH, HEIGHT);
        for selected in 0..3 {
            let view = MenuView {
                selected,
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
            let hero = crate::menus::HERO;
            let mut changed = 0;
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    if before.pixels[y * WIDTH + x] != after.pixels[y * WIDTH + x] {
                        assert!(
                            x >= hero.x
                                && x < hero.x + hero.width
                                && y >= hero.y
                                && y < hero.y + hero.height,
                            "La animacion debe permanecer dentro de la vista principal"
                        );
                        changed += 1;
                    }
                }
            }
            assert!(changed > 500, "La nave seleccionada debe animarse");
        }
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
