use crate::{
    framebuffer::Framebuffer,
    menus::glass,
    ui::{Rect, Ui, blend_pixel, rule},
};

pub const GAME_TITLE: &str = "Beyond the Interstellar";
pub const HANGAR_BACK: Rect = Rect::new(1048, 34, 180, 44);
pub const HELP_BACK: Rect = Rect::new(88, 742, 236, 44);
const TEXT: u32 = 0xf3eef7;
const MUTED: u32 = 0xb5aabd;
const ACCENT: u32 = 0xe5c6a1;
pub const OPTIONS: [&str; 5] = [
    "Seleccionar nave",
    "Elegir destino",
    "Controles",
    "Instrucciones",
    "Salir",
];

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Main,
    Controls,
    Instructions,
}

#[derive(Clone, Copy, Default)]
pub struct Home {
    pub selected: usize,
    pub page: Page,
}

pub enum Action {
    None,
    Ships,
    Destinations,
    Exit,
}

pub fn option(index: usize) -> Rect {
    Rect::new(88, 383 + index as i32 * 64, 470, 55)
}

impl Home {
    pub fn navigate(&mut self, direction: i32) {
        self.selected =
            (self.selected as i32 + direction).rem_euclid(OPTIONS.len() as i32) as usize;
    }

    pub fn hit(&self, point: (f32, f32)) -> Option<usize> {
        if self.page == Page::Main {
            (0..OPTIONS.len()).find(|&index| option(index).contains(point))
        } else {
            HELP_BACK.contains(point).then_some(0)
        }
    }

    pub fn activate(&mut self) -> Action {
        if self.page != Page::Main {
            self.page = Page::Main;
            return Action::None;
        }
        match self.selected {
            0 => Action::Ships,
            1 => Action::Destinations,
            2 => {
                self.page = Page::Controls;
                Action::None
            }
            3 => {
                self.page = Page::Instructions;
                Action::None
            }
            _ => Action::Exit,
        }
    }

    pub fn back(&mut self) {
        if self.page == Page::Main {
            self.selected = OPTIONS.len() - 1;
        } else {
            self.page = Page::Main;
        }
    }
}

pub fn draw(ui: &Ui, fb: &mut Framebuffer, home: &Home) {
    ui.begin(fb);
    // El velo conserva la nebulosa y aumenta el contraste de la tipografia.
    for y in 0..fb.height {
        for x in 0..780.min(fb.width) {
            let opacity = (1.0 - x as f32 / 780.0) * 0.40;
            blend_pixel(fb, x as i32, y as i32, 0x050209, opacity);
        }
    }
    if home.page == Page::Main {
        ui.fonts
            .text(fb, 14, 94, 77, "UN VIAJE MAS ALLA DE LO CONOCIDO", MUTED);
        ui.fonts
            .text_scaled(fb, (88, 118), "BEYOND THE", 1.18, TEXT);
        ui.fonts
            .text_scaled(fb, (84, 184), "INTERSTELLAR", 1.82, TEXT);
        rule(fb, (94, 321), (176, 321), ACCENT);
        ui.fonts
            .text(fb, 16, 94, 345, "Tu viaje empieza aqui.", MUTED);
        for (index, label) in OPTIONS.iter().enumerate() {
            let rect = option(index);
            let active = index == home.selected;
            if active {
                glass(fb, rect, 0x8a7469, 0.45);
                rule(fb, (106, rect.y + 18), (118, rect.y + 27), ACCENT);
                rule(fb, (118, rect.y + 27), (106, rect.y + 36), ACCENT);
                rule(fb, (96, rect.y + 27), (118, rect.y + 27), ACCENT);
            }
            ui.fonts.text(
                fb,
                28,
                142,
                rect.y + 11,
                label,
                if active { TEXT } else { MUTED },
            );
        }
        ui.fonts.text(
            fb,
            14,
            94,
            778,
            "Flechas / W-S Elegir    ENTER Confirmar    Clic Seleccionar",
            MUTED,
        );
    } else {
        let controls = home.page == Page::Controls;
        ui.fonts
            .text(fb, 14, 92, 50, "BEYOND THE INTERSTELLAR", ACCENT);
        ui.fonts.text(
            fb,
            52,
            88,
            100,
            if controls {
                "Controles"
            } else {
                "Como explorar"
            },
            TEXT,
        );
        ui.fonts.text(
            fb,
            18,
            92,
            176,
            if controls {
                "Todo lo que necesitas para encontrar tu rumbo."
            } else {
                "Elige tu nave, traza tu ruta y descubre otros mundos."
            },
            MUTED,
        );
        glass(fb, Rect::new(88, 232, 1104, 470), 0x5b4b64, 0.75);
        if controls {
            ui.fonts.text(fb, 22, 116, 259, "MENUS Y DESTINOS", ACCENT);
            ui.fonts.text(fb, 22, 660, 259, "VUELO ESPACIAL", ACCENT);
            let menus: &[(&str, &str)] = &[
                ("Flechas / TAB / Clic", "Seleccionar una opcion"),
                ("ENTER", "Confirmar o iniciar el viaje"),
                ("1 / 2 / 3", "Elegir nave o galaxia"),
                ("Rueda / + / -", "Acercar o alejar el mapa"),
                ("WASD / R", "Mover mapa / restablecer vista"),
                ("ESC", "Volver al menu anterior"),
            ];
            let flight: &[(&str, &str)] = &[
                ("Mouse", "Orientar la nave"),
                ("W / S", "Avanzar / retroceder"),
                ("A / D", "Desplazarse lateralmente"),
                ("SHIFT / ESPACIO", "Turbo / frenar"),
                ("I", "Inspeccionar la nave"),
                ("Clic izq / der", "Crucer: portal amarillo / verde"),
                ("Flechas / Q-E / M", "En inspeccion: orbita / zoom / vista"),
            ];
            for (column, rows) in [(116, menus), (660, flight)] {
                for (index, (keys, action)) in rows.iter().enumerate() {
                    let y = 311 + index as i32 * 51;
                    ui.fonts.text(fb, 18, column, y, keys, TEXT);
                    ui.fonts.text(fb, 16, column, y + 25, action, MUTED);
                }
            }
        } else {
            let steps = [
                (
                    "01",
                    "Prepara tu nave",
                    "Elige un modelo y observa su vista de giro antes de continuar.",
                ),
                (
                    "02",
                    "Descubre un destino",
                    "Pulsa una galaxia para ver sus mundos. Elige Viajar para despegar.",
                ),
                (
                    "03",
                    "Encuentra tu rumbo",
                    "La nave inicia detenida. Orientala con el mouse y avanza con W.",
                ),
                (
                    "04",
                    "Explora de cerca",
                    "Acercate a un planeta, frena y rodealo sin abandonar el vuelo.",
                ),
                (
                    "05",
                    "Cuida tu trayectoria",
                    "Los impactos fuertes destruyen la nave. El agujero negro puede atraerte.",
                ),
                (
                    "06",
                    "Conecta dos portales",
                    "Con Crucer: clic izquierdo amarillo, derecho verde. Atraviesa uno para saltar.",
                ),
            ];
            for (index, (number, title, detail)) in steps.iter().enumerate() {
                let y = 262 + index as i32 * 70;
                ui.fonts.text(fb, 28, 116, y + 4, number, ACCENT);
                ui.fonts.text(fb, 22, 184, y, title, TEXT);
                ui.fonts.text(fb, 18, 184, y + 31, detail, MUTED);
            }
        }
        glass(fb, HELP_BACK, 0x8a7469, 0.6);
        ui.fonts.text(fb, 18, 106, 754, "< Volver al inicio", TEXT);
        ui.fonts.text(
            fb,
            14,
            366,
            756,
            "ESC / ENTER Volver al menu principal",
            MUTED,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn home_navigation_opens_help_returns_and_dispatches_destinations() {
        let mut home = Home::default();
        home.navigate(-1);
        assert_eq!(home.selected, 4);
        assert!(matches!(home.activate(), Action::Exit));
        home.navigate(1);
        assert!(matches!(home.activate(), Action::Ships));
        home.navigate(1);
        assert!(matches!(home.activate(), Action::Destinations));
        home.navigate(1);
        home.activate();
        assert!(home.page == Page::Controls);
        home.back();
        assert!(home.page == Page::Main);
        home.navigate(1);
        home.activate();
        assert!(home.page == Page::Instructions);
        home.activate();
        assert!(home.page == Page::Main);
        for index in 0..OPTIONS.len() {
            let rect = option(index);
            assert_eq!(
                home.hit((rect.x as f32 + 15.0, rect.y as f32 + 20.0)),
                Some(index)
            );
        }
    }
}
