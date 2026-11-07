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
    ui.fonts
        .text(fb, 14, 978, 35, "01 / NAVE     02 / GALAXIA", MUTED);
    ui.fonts
        .text(fb, 14, 44, 126, "PREPARACION DE VUELO", ship.accent);
    ui.fonts
        .text(fb, 52, 40, 151, "Tu proximo horizonte.", TEXT);
    ui.fonts.text(fb, 16, 44, 220, "ELIGE UNA NAVE", MUTED);
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
        rounded(
            fb,
            card,
            12,
            mix(0x111e2d, 0x24374b, amount),
            mix(LINE, entry.accent, amount),
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
        mix(0x162538, ship.accent, 0.28),
    );
    ellipse(fb, (615.0, 640.0), (215.0, 19.0), 0x152537);
    renderer::draw_model(
        fb,
        &ship.model,
        &ui::preview_camera(view.selected, view.seconds),
        RenderMode::Solid,
        HERO,
        ship.finish,
    );
    ui.fonts
        .text(fb, 14, 340, 679, "VISTA ORBITAL / MODELO 3D", MUTED);
    rounded(
        fb,
        ui::PAUSE,
        8,
        if view.hover_pause { 0x2a3d50 } else { 0x142234 },
        LINE,
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
    rounded(fb, Rect::new(948, 255, 280, 438), 14, 0x101c2b, LINE);
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
    button(ui, fb, "ELEGIR GALAXIA", ship.accent, view.hover_button);
    ui.fonts.text(
        fb,
        14,
        44,
        815,
        "1-3 / Flechas Elegir   TAB Recorrer   ENTER Continuar   ESPACIO Pausar   ESC Salir",
        MUTED,
    );
}

pub fn galaxies(ui: &Ui, fb: &mut Framebuffer, ship: &Ship, view: &GalaxyView) {
    ui.begin(fb);
    let selected = &GALAXIES[view.selected];
    ui.fonts.text(
        fb,
        14,
        978,
        35,
        "01 / NAVE     02 / GALAXIA",
        selected.accent,
    );
    rounded(
        fb,
        ui::BACK,
        8,
        if view.hover_back { 0x2a3d50 } else { 0x142234 },
        LINE,
    );
    ui.fonts.text(fb, 16, 69, 121, "< Cambiar nave", TEXT);
    ui.fonts.text(fb, 52, 42, 150, "Atlas de destinos", TEXT);
    ui.fonts
        .text(fb, 14, 631, 176, "EXPLORACION / SECTOR 01", MUTED);
    rounded(fb, Rect::new(42, 230, 848, 479), 14, 0x080f1b, LINE);
    fb.set_clip(43, 231, 846, 477);
    for i in 0..440 {
        let x = 44 + ((i * 977 + 171) % 842);
        let y = 232 + ((i * 379 + 61) % 473);
        blend_pixel(fb, x, y, 0x99b8d8, if i % 11 == 0 { 0.55 } else { 0.18 });
    }
    let points: Vec<_> = (0..GALAXIES.len()).map(|i| view.point(i)).collect();
    rule(
        fb,
        (points[0].0 as i32, points[0].1 as i32),
        (points[1].0 as i32, points[1].1 as i32),
        0x284052,
    );
    rule(
        fb,
        (points[0].0 as i32, points[0].1 as i32),
        (points[2].0 as i32, points[2].1 as i32),
        0x284052,
    );
    for (index, galaxy) in GALAXIES.iter().enumerate() {
        let (cx, cy) = points[index];
        let active = index == view.selected;
        for i in 0..520 {
            let t = (i as f32 + 0.5) / 520.0;
            let arm = (i % 3) as f32 * TAU / 3.0;
            let angle = arm + t * 8.2 + view.angle + view.seconds * 0.025;
            let radius = t.sqrt() * 79.0 * view.zoom;
            let jitter = ((i * 71 % 37) as f32 - 18.0) * 0.20;
            let x = cx + angle.cos() * radius + jitter;
            let y = cy + angle.sin() * radius * 0.48 + jitter;
            blend_pixel(
                fb,
                x as i32,
                y as i32,
                galaxy.accent,
                (1.0 - t * 0.65) * if active { 0.95 } else { 0.5 },
            );
        }
        ellipse(fb, (cx, cy), (5.0, 5.0), galaxy.accent);
        if active || view.hover == Some(index) {
            for i in 0..96 {
                let angle = i as f32 * TAU / 96.0;
                let radius = 87.0 * view.zoom;
                blend_pixel(
                    fb,
                    (cx + angle.cos() * radius) as i32,
                    (cy + angle.sin() * radius * 0.62) as i32,
                    galaxy.accent,
                    0.8,
                );
            }
        }
        ui.fonts.text(
            fb,
            18,
            cx as i32 - 30,
            cy as i32 + (58.0 * view.zoom) as i32,
            galaxy.name,
            if active { TEXT } else { MUTED },
        );
    }
    fb.reset_clip();
    ui.fonts.text(
        fb,
        14,
        62,
        679,
        &format!(
            "ZOOM {:.0}%   /   3 DESTINOS CARTOGRAFIADOS",
            view.zoom * 100.0
        ),
        MUTED,
    );
    rounded(fb, Rect::new(918, 230, 310, 479), 14, 0x101c2b, LINE);
    ui.fonts
        .text(fb, 14, 940, 252, "DESTINO SELECCIONADO", selected.accent);
    ui.fonts.text(fb, 52, 938, 282, selected.name, TEXT);
    ui.fonts
        .text(fb, 14, 940, 353, selected.subtitle, selected.accent);
    rule(fb, (940, 389), (1206, 389), LINE);
    for (i, line) in selected.description.iter().enumerate() {
        ui.fonts.text(fb, 14, 940, 410 + i as i32 * 23, line, MUTED);
    }
    ui.fonts
        .text(fb, 14, 940, 505, "MUNDOS EN ESTA REGION", selected.accent);
    for (i, &planet) in selected.planets.iter().enumerate() {
        ui.fonts.text(
            fb,
            18,
            940,
            535 + i as i32 * 28,
            ["Siroco", "Oquedad", "Jovara", "Boreal"][planet],
            TEXT,
        );
    }
    ui.fonts.text(
        fb,
        14,
        940,
        677,
        if selected.debris == 0 {
            "Ciclon de arena / campo estelar"
        } else {
            "Cinturon de roca / campo estelar"
        },
        MUTED,
    );
    rule(fb, (42, 725), (1228, 725), LINE);
    ui.fonts.text(fb, 14, 44, 745, "NAVE PREPARADA", MUTED);
    ui.fonts.text(fb, 22, 44, 767, ship.name, TEXT);
    ui.fonts.text(
        fb,
        14,
        350,
        745,
        "Rueda / +/- Zoom   WASD Mover mapa   Q/E Girar   R Centrar",
        MUTED,
    );
    ui.fonts.text(
        fb,
        14,
        350,
        776,
        "1-3 / Flechas Elegir   ENTER Despegar   ESC Cambiar nave",
        MUTED,
    );
    button(ui, fb, "DESPEGAR", selected.accent, view.hover_button);
    ui.fonts.text(
        fb,
        14,
        44,
        815,
        "El destino define los planetas y la decoracion del vuelo.",
        MUTED,
    );
}
