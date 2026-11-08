use crate::{
    fleet::Ship,
    framebuffer::Framebuffer,
    galaxy::{GALAXIES, GalaxyView},
    renderer::{self, RenderMode, Viewport},
    ui::{self, MenuView, Rect, Ui, blend_pixel, ellipse, glow, mix, rounded, rule},
};
use std::f32::consts::TAU;

const TEXT: u32 = 0xf2f5fb;
const MUTED: u32 = 0xa0aec2;
const LINE: u32 = 0x283b50;
pub const HERO: Viewport = Viewport {
    x: 310,
    y: 247,
    width: 610,
    height: 420,
};

pub const DESTINATION_PANEL: Rect = Rect::new(922, 375, 314, 334);
pub const DESTINATION_CLOSE: Rect = Rect::new(1194, 391, 24, 24);
pub const DESTINATION_BACK: Rect = Rect::new(44, 740, 190, 46);

pub(crate) fn glass(fb: &mut Framebuffer, rect: Rect, border: u32, opacity: f32) {
    let radius = 12.0;
    for y in rect.y.max(0)..(rect.y + rect.h).min(fb.height as i32) {
        for x in rect.x.max(0)..(rect.x + rect.w).min(fb.width as i32) {
            let dx = (x as f32 - (rect.x as f32 + radius)).min(0.0)
                + (x as f32 - (rect.x + rect.w) as f32 + radius + 1.0).max(0.0);
            let dy = (y as f32 - (rect.y as f32 + radius)).min(0.0)
                + (y as f32 - (rect.y + rect.h) as f32 + radius + 1.0).max(0.0);
            let distance = dx * dx + dy * dy;
            if distance <= radius * radius {
                let edge = distance > (radius - 1.0) * (radius - 1.0)
                    || x == rect.x
                    || x == rect.x + rect.w - 1
                    || y == rect.y
                    || y == rect.y + rect.h - 1;
                blend_pixel(
                    fb,
                    x,
                    y,
                    if edge { border } else { 0x07050d },
                    if edge { 0.65 } else { opacity },
                );
            }
        }
    }
}

fn button(ui: &Ui, fb: &mut Framebuffer, label: &str, accent: u32, hover: bool) {
    rounded(
        fb,
        ui::LAUNCH,
        10,
        if hover {
            mix(accent, 0xffffff, 0.3)
        } else {
            accent
        },
        accent,
    );
    let x = ui::LAUNCH.x + (ui::LAUNCH.w - ui.fonts.width(16, label) as i32) / 2;
    ui.fonts.text(fb, 16, x, ui::LAUNCH.y + 17, label, 0x081b24);
}

pub fn ships(ui: &Ui, fb: &mut Framebuffer, ships: &[Ship], view: &MenuView) {
    ui.begin(fb);
    let ship = &ships[view.selected];
    glass(fb, crate::home::HANGAR_BACK, LINE, 0.55);
    ui.fonts.text(fb, 16, 1062, 48, "< Menu principal", TEXT);
    ui.fonts.text(fb, 28, 44, 36, "Selecciona tu nave", TEXT);
    ui.fonts.text(fb, 52, 40, 140, ship.name, TEXT);
    ui.fonts.text(fb, 14, 44, 220, "FLOTA DISPONIBLE", MUTED);
    let descriptions = [
        "Casco cobrizo y silueta amplia",
        "Paneles oscuros y cabina roja",
        "Perfil compacto y tonos frios",
    ];
    for (index, entry) in ships.iter().enumerate() {
        let card = ui::card(index);
        let active = index == view.selected;
        let amount = view.emphasis[index].max(if active {
            0.8
        } else if view.hover == Some(index) {
            0.3
        } else {
            0.0
        });
        glass(
            fb,
            card,
            mix(0x54465a, entry.accent, amount),
            0.42 + amount * 0.16,
        );
        if active {
            rounded(
                fb,
                Rect::new(card.x + 1, card.y + 20, 3, 72),
                1,
                entry.accent,
                entry.accent,
            );
        }
        renderer::draw_model(
            fb,
            &entry.model,
            &ui::preview_camera(index, 0.0),
            RenderMode::Solid,
            Viewport {
                x: 52,
                y: (card.y + 18) as usize,
                width: 80,
                height: 76,
            },
            entry.finish,
        );
        ui.fonts.text(
            fb,
            14,
            144,
            card.y + 22,
            &format!(
                "0{} / {}",
                index + 1,
                if active { "ACTIVA" } else { "DISPONIBLE" }
            ),
            if active { entry.accent } else { MUTED },
        );
        ui.fonts.text(fb, 18, 144, card.y + 49, entry.name, TEXT);
        ui.fonts
            .text(fb, 14, 144, card.y + 79, "Ver modelo  >", MUTED);
    }
    glow(fb, (610.0, 476.0), (300.0, 220.0), ship.accent, 0.10);
    ellipse(
        fb,
        (615.0, 640.0),
        (245.0, 28.0),
        mix(0x17101e, ship.accent, 0.28),
    );
    ellipse(fb, (615.0, 640.0), (215.0, 19.0), 0x110c17);
    renderer::draw_model(
        fb,
        &ship.model,
        &ui::preview_camera(view.selected, view.seconds),
        RenderMode::Solid,
        HERO,
        ship.finish,
    );
    ui.fonts.text(fb, 14, 340, 679, "VISTA 360°", MUTED);
    glass(
        fb,
        ui::PAUSE,
        LINE,
        if view.hover_pause { 0.75 } else { 0.45 },
    );
    ui.fonts.text(
        fb,
        16,
        1048,
        153,
        if view.paused {
            "Reanudar giro"
        } else {
            "Pausar giro"
        },
        TEXT,
    );
    glass(fb, Rect::new(948, 255, 280, 438), LINE, 0.68);
    ui.fonts
        .text(fb, 14, 970, 279, "FICHA DE LA NAVE", ship.accent);
    ui.fonts.text(fb, 28, 970, 310, ship.name, TEXT);
    rule(fb, (970, 365), (1206, 365), LINE);
    let rows = [
        (
            "VELOCIDAD / TURBO",
            format!(
                "{:.0} / {:.0} u/s",
                crate::flight::CRUISE_SPEED,
                crate::flight::BOOST_SPEED
            ),
        ),
        ("TRIANGULOS", format!("{}", ship.model.faces.len())),
        ("VERTICES", format!("{}", ship.model.vertices.len())),
        (
            "RADIO DE COLISION",
            format!("{:.1} u", crate::collision::SHIP_RADIUS),
        ),
    ];
    for (index, (label, value)) in rows.iter().enumerate() {
        let y = 385 + index as i32 * 63;
        ui.fonts.text(fb, 14, 970, y, label, MUTED);
        ui.fonts.text(fb, 22, 970, y + 20, value, TEXT);
    }
    ui.fonts
        .text(fb, 14, 970, 658, "Propulsion compartida", MUTED);
    rule(fb, (42, 718), (1228, 718), LINE);
    ui.fonts
        .text(fb, 18, 44, 744, descriptions[view.selected], TEXT);
    ui.fonts.text(
        fb,
        14,
        44,
        775,
        "Selecciona tu destino en el siguiente paso.",
        MUTED,
    );
    button(ui, fb, "ELEGIR DESTINO", ship.accent, view.hover_button);
    ui.fonts.text(
        fb,
        14,
        44,
        815,
        "1-3 / Flechas Elegir   TAB Recorrer   ENTER Continuar   ESPACIO Pausar   ESC Inicio",
        MUTED,
    );
}

pub fn galaxies(ui: &Ui, fb: &mut Framebuffer, ship: &Ship, view: &GalaxyView) {
    ui.begin_destinations(fb);
    let selected = &GALAXIES[view.selected];
    let points: Vec<_> = (0..GALAXIES.len()).map(|i| view.point(i)).collect();
    for index in 1..points.len() {
        let (a, b) = (points[0], points[index]);
        for step in 0..80 {
            let t = step as f32 / 80.0;
            blend_pixel(
                fb,
                (a.0 + (b.0 - a.0) * t) as i32,
                (a.1 + (b.1 - a.1) * t) as i32,
                0xb1a2cb,
                0.25,
            );
        }
    }
    for (index, galaxy) in GALAXIES.iter().enumerate() {
        let (cx, cy) = points[index];
        let active = view.details_open && index == view.selected;
        crate::galaxy_art::draw(fb, galaxy.artwork, (cx, cy), view.zoom, view.seconds);
        if active || view.hover == Some(index) {
            for i in 0..160 {
                let angle = i as f32 * TAU / 160.0;
                let radius = 210.0 * view.zoom;
                blend_pixel(
                    fb,
                    (cx + angle.cos() * radius) as i32,
                    (cy + angle.sin() * radius * 0.62) as i32,
                    galaxy.accent,
                    0.65,
                );
            }
        }
        let label_width = ui.fonts.width(22, galaxy.name);
        let label_x = (cx - label_width * 0.5) as i32;
        let label_y = cy as i32 + (119.0 * view.zoom) as i32;
        let covered = view.details_open
            && label_x < DESTINATION_PANEL.x + DESTINATION_PANEL.w
            && label_x + label_width as i32 > DESTINATION_PANEL.x
            && label_y < DESTINATION_PANEL.y + DESTINATION_PANEL.h
            && label_y + 28 > DESTINATION_PANEL.y;
        if !covered {
            ui.fonts.text(fb, 22, label_x, label_y, galaxy.name, TEXT);
        }
    }
    ui.fonts.text(fb, 52, 42, 31, "Destinos", TEXT);
    ui.fonts.text(
        fb,
        16,
        46,
        100,
        "Selecciona una galaxia para descubrir sus mundos.",
        MUTED,
    );
    if view.details_open {
        glass(fb, DESTINATION_PANEL, selected.accent, 0.93);
        ui.fonts.text(fb, 28, 942, 395, selected.name, TEXT);
        rule(fb, (1200, 397), (1211, 408), MUTED);
        rule(fb, (1200, 408), (1211, 397), MUTED);
        ui.fonts
            .text(fb, 14, 942, 438, selected.subtitle, selected.accent);
        for (i, line) in selected.description.iter().enumerate() {
            ui.fonts.text(fb, 14, 942, 476 + i as i32 * 23, line, MUTED);
        }
        ui.fonts.text(
            fb,
            14,
            942,
            560,
            if view.selected == 1 {
                "SOL, PLANETAS Y LUNA"
            } else {
                "MUNDOS POR EXPLORAR"
            },
            selected.accent,
        );
        for (i, &planet) in selected.planets.iter().enumerate() {
            let name = match planet {
                4 => "Helios II (sol)",
                7 => "Metis (luna de Kepler)",
                _ => crate::galaxy::BODY_NAMES[planet],
            };
            ui.fonts.text(fb, 16, 942, 582 + i as i32 * 20, name, TEXT);
        }
        button(ui, fb, "VIAJAR", selected.accent, view.hover_button);
    }
    glass(
        fb,
        DESTINATION_BACK,
        0x62526e,
        if view.hover_back { 0.8 } else { 0.5 },
    );
    ui.fonts.text(fb, 16, 61, 755, "< Cambiar nave", TEXT);
    let label = format!("{}  /  {:.0}%", ship.name, view.zoom * 100.0);
    ui.fonts.text(fb, 14, 270, 754, &label, MUTED);
    ui.fonts.text(fb, 14, 44, 815, "Clic / 1-3 Elegir   Flechas / TAB Recorrer   Rueda Zoom   WASD Mover   R Centrar   ENTER Continuar   ESC Volver", MUTED);
}
