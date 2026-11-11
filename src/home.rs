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

fn wrapped_text(ui: &Ui, fb: &mut Framebuffer, origin: (i32, i32), width: f32, text: &str) {
    let mut line = String::new();
    let mut y = origin.1;
    // El salto se calcula con el ancho real de la fuente para conservar el margen del panel.
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if !line.is_empty() && ui.fonts.width(16, &candidate) > width {
            ui.fonts.text(fb, 16, origin.0, y, &line, MUTED);
            y += 21;
            line = word.to_owned();
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        ui.fonts.text(fb, 16, origin.0, y, &line, MUTED);
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
        fb.set_clip(106, 244, 1068, 442);
        if controls {
            let columns: [(&str, &[(&str, &str)]); 3] = [
                (
                    "MENUS Y DESTINOS",
                    &[
                        ("Flechas / W-S / TAB", "Seleccionar una opcion"),
                        ("SHIFT + TAB", "Seleccionar la opcion anterior"),
                        ("ENTER / Clic", "Abrir, confirmar o iniciar viaje"),
                        ("1 / 2 / 3", "Elegir nave o destino"),
                        ("Rueda / + / -", "Acercar o alejar el mapa"),
                        ("WASD / R", "Mover mapa / restablecer vista"),
                        ("ESPACIO", "Pausar el giro en el hangar"),
                        ("ESC", "Volver o cancelar el viaje"),
                    ],
                ),
                (
                    "VUELO Y SUPERFICIE",
                    &[
                        ("C", "Alternar mouse y flechas"),
                        ("Mouse / Flechas", "Orientar con el modo elegido"),
                        ("W / S", "Avanzar / retroceder"),
                        ("A / D", "Desplazarse lateralmente"),
                        ("SHIFT / ESPACIO", "Turbo / frenar"),
                        ("L", "Activar o salir del modo local"),
                        ("Q / E", "Modo local: bajar / subir"),
                        ("I / 1-2-3", "Inspeccionar / cambiar nave"),
                    ],
                ),
                (
                    "PORTALES Y HERRAMIENTAS",
                    &[
                        (
                            "Clic izquierdo / derecho",
                            "Crucer: portal amarillo / verde",
                        ),
                        ("G", "Crucer: abrir portal galactico"),
                        ("Flechas / TAB / ENTER", "Portal galactico: elegir / entrar"),
                        ("CAPS LOCK + WASD", "TIE: impulso en la direccion elegida"),
                        ("F4 / F5", "Alternar LOD automatico / descarte"),
                        ("F6", "Pausar o reanudar el vuelo"),
                        ("Flechas-WASD / Q-E", "Inspeccion: orbitar / zoom"),
                        ("M / R / P", "Inspeccion: vista / reinicio; P: foto"),
                    ],
                ),
            ];
            for (index, (title, rows)) in columns.iter().enumerate() {
                let x = 116 + index as i32 * 360;
                ui.fonts.text(fb, 18, x, 258, title, ACCENT);
                for (row, (keys, action)) in rows.iter().enumerate() {
                    let y = 303 + row as i32 * 48;
                    ui.fonts.text(fb, 16, x, y, keys, TEXT);
                    ui.fonts.text(fb, 14, x, y + 23, action, MUTED);
                }
            }
        } else {
            let steps = [
                (
                    "01",
                    "Prepara tu nave",
                    "Selecciona Benatar, TIE Silencer o Crucer espacial. Confirma para elegir un destino.",
                ),
                (
                    "02",
                    "Elige una galaxia",
                    "Selecciona un destino para leer su descripcion. Pulsa Viajar o ENTER; ESC cancela el viaje.",
                ),
                (
                    "03",
                    "Controla el vuelo",
                    "C alterna mouse y flechas. WASD mueve; SHIFT da turbo y ESPACIO frena. Con TIE, CAPS LOCK + WASD da un impulso; sin WASD, hacia adelante.",
                ),
                (
                    "04",
                    "Rodea un planeta",
                    "Acercate y pulsa L para activar el modo local. WASD rodea la superficie; Q/E baja o sube. L permite salir.",
                ),
                (
                    "05",
                    "Siente la gravedad",
                    "Los cuerpos celestes desvian el vuelo. El modo local compensa la atraccion. Frena: los impactos fuertes destruyen la nave.",
                ),
                (
                    "06",
                    "Conecta dos portales",
                    "Con Crucer, usa clic izquierdo para el amarillo y derecho para el verde. Colocalos en espacio libre y atraviesa uno.",
                ),
                (
                    "07",
                    "Salta a otra galaxia",
                    "Con Crucer, G abre el portal galactico. Elige con flechas, TAB o clic; G o ENTER inicia el salto. ESC permite cancelar.",
                ),
                (
                    "08",
                    "Compara el rendimiento",
                    "F6 congela el vuelo. F4 alterna LOD automatico y detalle alto; F5 alterna descarte. Compara los triangulos procesados.",
                ),
            ];
            for (index, (number, title, detail)) in steps.iter().enumerate() {
                let x = 116 + (index / 4) as i32 * 544;
                let y = 258 + (index % 4) as i32 * 106;
                ui.fonts.text(fb, 28, x, y, number, ACCENT);
                ui.fonts.text(fb, 22, x + 55, y, title, TEXT);
                wrapped_text(ui, fb, (x + 55, y + 32), 445.0, detail);
            }
        }
        fb.reset_clip();
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
