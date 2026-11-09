pub fn is_active(window: &mut minifb::Window) -> bool {
    normalize_focus(window.is_active(), cfg!(target_os = "macos"))
}

fn normalize_focus(reported_active: bool, macos: bool) -> bool {
    // minifb 0.26: OSXWindow almacena true al recibir el foco, pero el wrapper
    // de Rust compara mfb_is_active(...) == 0. Los otros backends no lo invierten.
    if macos {
        !reported_active
    } else {
        reported_active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focused_and_unfocused_windows_are_normalized_for_each_backend() {
        assert!(normalize_focus(false, true), "macOS: ventana enfocada");
        assert!(!normalize_focus(true, true), "macOS: ventana sin foco");
        assert!(
            normalize_focus(true, false),
            "Windows/Linux: ventana enfocada"
        );
        assert!(
            !normalize_focus(false, false),
            "Windows/Linux: ventana sin foco"
        );
    }

    #[test]
    fn macos_travel_pauses_without_focus_and_finishes_after_focus_returns() {
        let mut travel = crate::travel::Travel::new(1);
        for _ in 0..60 {
            if normalize_focus(true, true) {
                travel.advance(1.0 / 60.0);
            }
        }
        assert_eq!(travel.progress(), 0.0);
        for _ in 0..180 {
            if normalize_focus(false, true) {
                travel.advance(1.0 / 60.0);
            }
        }
        assert_eq!(travel.progress(), 1.0);
    }
}
