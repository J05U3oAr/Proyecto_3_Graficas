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
pub const GATE_CHOICES: [Rect; 2] = [Rect::new(376, 727, 238, 51), Rect::new(630, 727, 238, 51)];
pub const GATE_ENTER: Rect = Rect::new(964, 727, 264, 51);
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
    pub(crate) fn text_scaled(
        &self,
        fb: &mut Framebuffer,
        position: (i32, i32),
        text: &str,
        scale: f32,
        color: u32,
    ) {
        let size = 52;
        let mut cursor = position.0 as f32;
        for character in text.chars() {
            let glyph = self.glyph(size, character);
            let origin_x = cursor + glyph.left as f32 * scale;
            let origin_y = position.1 as f32 + (size as i32 + glyph.top) as f32 * scale;
            for y in 0..(glyph.h as f32 * scale).ceil() as u32 {
                for x in 0..(glyph.w as f32 * scale).ceil() as u32 {
                    let source_x = (x as f32 / scale).min(glyph.w.saturating_sub(1) as f32);
                    let source_y = (y as f32 / scale).min(glyph.h.saturating_sub(1) as f32);
                    let left = source_x.floor() as u32;
                    let top = source_y.floor() as u32;
                    let right = (left + 1).min(glyph.w.saturating_sub(1));
                    let bottom = (top + 1).min(glyph.h.saturating_sub(1));
                    let sample = |x, y| self.atlas.get_pixel(glyph.x + x, glyph.y + y)[3] as f32;
                    let u = source_x - left as f32;
                    let v = source_y - top as f32;
                    let upper = sample(left, top) * (1.0 - u) + sample(right, top) * u;
                    let lower = sample(left, bottom) * (1.0 - u) + sample(right, bottom) * u;
                    let alpha = (upper * (1.0 - v) + lower * v) / 255.0;
                    if alpha > 0.0 {
                        blend_pixel(
                            fb,
                            origin_x.round() as i32 + x as i32,
                            origin_y.round() as i32 + y as i32,
                            color,
                            alpha,
                        );
                    }
                }
            }
            cursor += glyph.advance * scale;
        }
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
    background: &'static [u32],
    stars: Vec<Star>,
    space: crate::space::Space,
    deep_space: crate::deep_space::DeepSpace,
    portal_art: &'static crate::portal_art::Art,
    galaxy: usize,
}

struct Star {
    position: Vec3,
    phase: f32,
    color: u32,
}
impl Ui {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        crate::galaxy_art::prepare();
        crate::galactic_portal::prepare();
        let background = &crate::nebula::backgrounds().hangar;
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
            background,
            stars,
            space: crate::space::Space::load()?,
            deep_space: crate::deep_space::DeepSpace::new(),
            portal_art: crate::portal_art::Art::load()?,
        })
    }

    pub(crate) fn begin(&self, fb: &mut Framebuffer) {
        fb.clear();
        fb.pixels.copy_from_slice(self.background);
    }

    pub(crate) fn begin_destinations(&self, fb: &mut Framebuffer) {
        fb.clear();
        fb.pixels
            .copy_from_slice(&crate::nebula::backgrounds().destinations);
    }

    pub fn travel(
        &self,
        fb: &mut Framebuffer,
        travel: &crate::travel::Travel,
        arrival: &Framebuffer,
    ) {
        crate::travel::draw(
            fb,
            travel,
            &crate::nebula::backgrounds().destinations,
            &arrival.pixels,
        );
        if travel.progress() < 0.75 {
            let name = crate::galaxy::GALAXIES[travel.destination].name;
            let label = format!("RUMBO A {}", name.to_uppercase());
            let x = (fb.width as f32 - self.fonts.width(22, &label)) as i32 / 2;
            self.fonts.text(fb, 22, x, 735, &label, 0xe8e7ff);
        }
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
        if index == 1 {
            crate::skybox::prepare();
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

    fn scene_in_view(
        &self,
        fb: &mut Framebuffer,
        view: &renderer::FlightView,
        ship: &Ship,
        flight: &Flight,
        seconds: f32,
        show_ship: bool,
    ) {
        fb.clear();
        if self.galaxy == 1 {
            crate::skybox::draw_view(fb, view);
        } else {
            for y in 0..fb.height {
                let full_y = y as f32 + HEIGHT as f32 * 0.5 - view.projection_center().y;
                let color = mix(
                    crate::galaxy::GALAXIES[self.galaxy].sky,
                    0x03060c,
                    full_y / HEIGHT as f32,
                );
                fb.pixels[y * fb.width..(y + 1) * fb.width].fill(color);
            }
        }
        for star in &self.stars {
            let relative =
                (star.position - view.eye()).map(|v| (v + 280.0).rem_euclid(560.0) - 280.0);
            let Some(point) = view.project(view.eye() + relative) else {
                continue;
            };
            if !(3.0..280.0).contains(&(1.0 / point.z)) {
                continue;
            }
            let twinkle = 0.65 + ((seconds * 1.7 + star.phase).sin() + 1.0) * 0.16;
            blend_pixel(
                fb,
                point.x.round() as i32,
                point.y.round() as i32,
                mix(0x26364a, star.color, twinkle),
                0.8,
            );
        }
        self.space.draw_view(fb, view, seconds);
        self.portal_art.draw_view(fb, flight, view);
        if show_ship && flight.crash.is_none() {
            renderer::draw_ship_in_view(fb, ship, flight, view, self.space.sun_position(seconds));
        }
        self.deep_space
            .draw_view(fb, view, flight.black_hole, seconds);
        self.space.draw_atmospheres_view(fb, view, seconds);
        if let Some(crash) = &flight.crash {
            crate::explosion::draw(fb, view, crash);
        }
    }

    pub fn flight(&self, fb: &mut Framebuffer, ship: &Ship, flight: &Flight, seconds: f32) {
        fb.clear();
        if self.galaxy == 1 {
            crate::skybox::draw(fb, &flight.camera);
        } else {
            for y in 0..HEIGHT {
                let t = y as f32 / HEIGHT as f32;
                let color = mix(crate::galaxy::GALAXIES[self.galaxy].sky, 0x03060c, t);
                fb.pixels[y * WIDTH..(y + 1) * WIDTH].fill(color);
            }
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
        self.portal_art.draw_linked(fb, flight, |image, view| {
            self.scene_in_view(image, view, ship, flight, seconds, true);
        });
        if flight.crash.is_none() {
            if let Some(solar) = self.space.solar_lighting(seconds) {
                renderer::draw_flight_model_lit(
                    fb,
                    &ship.model,
                    flight,
                    ship.finish,
                    Some(&solar),
                    Some(&ship.shadows),
                    Some(&ship.lights),
                );
            } else {
                renderer::draw_flight_model(
                    fb,
                    &ship.model,
                    flight,
                    ship.finish,
                    Some(&ship.lights),
                );
            }
        }
        self.deep_space.draw(fb, flight);
        crate::thrusters::draw(fb, ship, flight, seconds);
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

        if flight.portals.flash > 0.0 {
            let alpha = (flight.portals.flash / 0.45).clamp(0.0, 1.0) * 0.16;
            fb.shade_rect((0, 0, WIDTH, HEIGHT), |_, _, pixel, _| {
                *pixel = mix(*pixel, 0xc6f77b, alpha);
            });
        }

        let view = renderer::FlightView::new(flight, WIDTH, HEIGHT);
        let mut occupied_labels: Vec<Rect> = Vec::new();
        let has_sun = self.space.sun_position(seconds).is_some();
        let hole = flight.black_hole;
        let distance = (flight.position - hole.position).norm();
        if let Some(point) = view.project(hole.position) {
            let y = point.y + hole.horizon * renderer::FLIGHT_FOCAL * point.z + 22.0;
            let label = format!("Agujero negro  {:.0} u", distance);
            if (180.0..WIDTH as f32 - 180.0).contains(&point.x) && (145.0..650.0).contains(&y) {
                occupied_labels.push(Rect::new(
                    (point.x - self.fonts.width(14, &label) * 0.5) as i32,
                    y as i32,
                    self.fonts.width(14, &label) as i32,
                    19,
                ));
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
            let transform = planet.world_transform(seconds);
            if let Some(point) = view.project(transform.position) {
                let atmosphere = if planet.dust.is_some() { 1.22 } else { 1.0 };
                let y = point.y
                    + transform.radius * atmosphere * renderer::FLIGHT_FOCAL * point.z
                    + 14.0;
                if (0.0..WIDTH as f32).contains(&point.x) && (145.0..710.0).contains(&y) {
                    // A nearby portal can cover a distant planet and its label anchor.
                    let anchor = y as usize * WIDTH + point.x as usize;
                    if fb.depth[anchor] > point.z {
                        continue;
                    }
                    let label = if has_sun {
                        format!(
                            "{}  {:.0} u",
                            planet.name,
                            (transform.position - flight.position).norm()
                        )
                    } else {
                        planet.name.to_owned()
                    };
                    let width = self.fonts.width(14, &label).ceil() as i32;
                    let x = (point.x - width as f32 * 0.5)
                        .clamp(16.0, (WIDTH as i32 - width - 16) as f32)
                        as i32;
                    let mut rect = Rect::new(x, y as i32, width, 19);
                    for _ in 0..5 {
                        let overlaps = occupied_labels.iter().any(|other| {
                            rect.x < other.x + other.w
                                && rect.x + rect.w > other.x
                                && rect.y < other.y + other.h
                                && rect.y + rect.h > other.y
                        });
                        if !overlaps {
                            break;
                        }
                        rect.y += 21;
                    }
                    if rect.y < 740 {
                        self.fonts.text(fb, 14, rect.x, rect.y, &label, MUTED);
                        occupied_labels.push(rect);
                    }
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
        if ship.portal_capable {
            rounded(fb, Rect::new(34, 258, 340, 42), 10, 0x152139, 0x6653a0);
            self.fonts
                .text(fb, 16, 52, 271, "G  /  PORTAL GALACTICO", 0xb5b9ff);
            rounded(fb, Rect::new(34, 107, 340, 137), 12, 0x101d2c, 0x304154);
            self.fonts.text(fb, 16, 52, 121, "PORTALES", TEXT);
            self.fonts.text(
                fb,
                14,
                210,
                122,
                if flight.portals.linked() {
                    "ENLAZADOS"
                } else {
                    "INCOMPLETOS"
                },
                MUTED,
            );
            for (i, kind) in crate::portal::Kind::ALL.into_iter().enumerate() {
                let y = 152 + i as i32 * 28;
                let button = if kind == crate::portal::Kind::Yellow {
                    "IZQ"
                } else {
                    "DER"
                };
                self.fonts.text(
                    fb,
                    14,
                    52,
                    y,
                    &format!("{button} / {}", kind.label()),
                    kind.color(),
                );
                let state = flight.portals.get(kind).map_or_else(
                    || "SIN COLOCAR".to_owned(),
                    |portal| format!("LISTO / {:.0} u", (portal.center - flight.position).norm()),
                );
                self.fonts.text(fb, 14, 218, y, &state, TEXT);
            }
            self.fonts
                .text(fb, 14, 52, 216, "Apunta con el mouse y haz clic.", MUTED);
            if let Some(notice) = flight.portals.notice {
                rounded(fb, Rect::new(390, 121, 554, 43), 10, 0x152331, 0x536940);
                self.fonts.text(fb, 14, 406, 135, notice, TEXT);
            }
        }
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

    pub fn galaxy_index(&self) -> usize {
        self.galaxy
    }

    pub fn capture_galactic_arrival(
        &mut self,
        ship: &Ship,
        destination: usize,
        image: &mut Framebuffer,
    ) {
        let origin = self.galaxy;
        self.select_galaxy(destination);
        let mut flight = self.new_flight();
        flight.portals.set_enabled(ship.portal_capable);
        self.flight(image, ship, &flight, 0.0);
        self.select_galaxy(origin);
    }

    pub fn galactic_gate(
        &self,
        fb: &mut Framebuffer,
        ship: &Ship,
        source: &Flight,
        jump: &crate::galactic_portal::Jump,
        arrival: &Framebuffer,
    ) {
        if jump.progress() >= 1.0 {
            fb.pixels.copy_from_slice(&arrival.pixels);
            fb.depth.copy_from_slice(&arrival.depth);
            return;
        }
        let (pose, view) = jump.cinematic(source, fb.width, fb.height);
        self.scene_in_view(fb, &view, ship, &pose, source.seconds, jump.ship_visible());
        crate::galactic_portal::draw(fb, jump, &view);
        crate::galactic_portal::finish(fb, jump, arrival);
        if jump.progress() >= 0.80 {
            return;
        }
        crate::menus::glass(fb, Rect::new(34, 25, 436, 105), 0x6653a0, 0.85);
        self.fonts
            .text(fb, 14, 54, 40, "SALTO INTERGALACTICO", 0xb5b9ff);
        self.fonts.text(
            fb,
            28,
            52,
            68,
            &format!(
                "{} > {}",
                crate::galaxy::GALAXIES[jump.origin].name,
                crate::galaxy::GALAXIES[jump.destination].name
            ),
            TEXT,
        );
        crate::menus::glass(fb, Rect::new(34, 708, 1194, 108), 0x6653a0, 0.9);
        self.fonts.text(fb, 18, 54, 735, jump.stage(), TEXT);
        if !jump.entering() {
            let destinations = [(jump.origin + 1) % 3, (jump.origin + 2) % 3];
            for (i, destination) in destinations.into_iter().enumerate() {
                let active = destination == jump.destination;
                let rect = GATE_CHOICES[i];
                rounded(
                    fb,
                    rect,
                    8,
                    if active { 0x30304f } else { 0x151d2f },
                    if active { 0x9cf3ff } else { BORDER },
                );
                self.fonts.text(
                    fb,
                    22,
                    rect.x + 20,
                    rect.y + 12,
                    crate::galaxy::GALAXIES[destination].name,
                    TEXT,
                );
            }
            rounded(
                fb,
                GATE_ENTER,
                8,
                if jump.ready() { 0x9cf3ff } else { 0x485463 },
                0x9cf3ff,
            );
            self.fonts.text(
                fb,
                18,
                GATE_ENTER.x + 26,
                GATE_ENTER.y + 15,
                if jump.ready() {
                    "G / ENTER  ENTRAR"
                } else {
                    "ABRIENDO..."
                },
                0x101725,
            );
            self.fonts.text(
                fb,
                14,
                54,
                790,
                "Flechas: elegir galaxia    |    G / ENTER: entrar    |    ESC: cancelar",
                MUTED,
            );
        } else {
            self.fonts
                .text(fb, 14, 54, 786, "ESC: cancelar salto", MUTED);
            rounded(fb, Rect::new(470, 746, 738, 10), 5, 0x27344a, 0x27344a);
            rounded(
                fb,
                Rect::new(470, 746, (738.0 * jump.progress()) as i32, 10),
                5,
                0x9cf3ff,
                0x9cf3ff,
            );
        }
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
    fn portal_shows_the_destination_planet_and_its_current_animation_without_advancing_flight() {
        use crate::portal::{Kind, Portal};
        let ui = Ui::new().unwrap();
        let ships = crate::fleet::load().unwrap();
        let mut flight = ui.new_flight();
        flight.camera.pitch = 0.0;
        flight.portals.set_enabled(true);
        let entry = Portal::new(
            flight.camera_position() + flight.camera.basis().2 * 40.0,
            -flight.camera.basis().2,
        )
        .unwrap();
        let planet = ui
            .space
            .active_planets()
            .find(|p| p.name == "Boreal")
            .unwrap()
            .world_transform(0.0);
        let exit = Portal::new(
            planet.position + Vec3::z() * (planet.radius + 250.0),
            -Vec3::z(),
        )
        .unwrap();
        flight.portals.place(Kind::Yellow, entry);
        flight.portals.place(Kind::Green, exit);
        let original = flight.position;
        let mut before = Framebuffer::new(WIDTH, HEIGHT);
        let mut after = Framebuffer::new(WIDTH, HEIGHT);
        ui.flight(&mut before, &ships[2], &flight, 0.0);
        ui.flight(&mut after, &ships[2], &flight, 3.0);
        let area: Vec<_> = (370..470)
            .flat_map(|y| (590..690).map(move |x| y * WIDTH + x))
            .collect();
        let blue = area
            .iter()
            .filter(|&&i| {
                let p = before.pixels[i];
                (p & 255) > (p >> 16 & 255) + 30 && (p >> 8 & 255) > (p >> 16 & 255) + 30
            })
            .count();
        assert!(
            blue > 1000,
            "The exit faces Boreal, not a yellow swirl: {blue}"
        );
        assert!(
            area.iter()
                .filter(|&&i| before.pixels[i] != after.pixels[i])
                .count()
                > 200
        );
        assert_eq!(flight.position, original);
        assert_eq!(flight.seconds, 0.0);
        assert_eq!(flight.portals.traversals, 0);
        assert_eq!(flight.portals.get(Kind::Green).unwrap().center, exit.center);
    }
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
