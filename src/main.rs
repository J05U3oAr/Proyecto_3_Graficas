mod camera;
mod color;
mod fleet;
mod framebuffer;
mod model;
mod renderer;
mod triangle;
mod ui;

use camera::Camera;
use framebuffer::Framebuffer;
use minifb::{
    CursorStyle, Key, KeyRepeat, MouseButton, MouseMode, ScaleMode, Window, WindowOptions,
};
use renderer::RenderMode;
use std::{error::Error, path::Path, time::Instant};
use ui::{HEIGHT, WIDTH};

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Menu,
    Inspector,
}

#[derive(Default)]
struct AnimationClock {
    seconds: f32,
    paused: bool,
}
impl AnimationClock {
    fn advance(&mut self, dt: f32) {
        if !self.paused {
            self.seconds += dt;
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut ships = fleet::load()?;
    let ui = ui::Ui::new()?;
    let mut framebuffer = Framebuffer::new(WIDTH, HEIGHT);
    // Deterministic exports let us inspect the CPU-rendered menu without a window.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        let mut output = None;
        let mut seconds = 0.0f32;
        let mut selected = 0usize;
        let mut inspector = false;
        let mut args = args.iter();
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--render-menu" | "--render-inspector" => {
                    inspector = argument == "--render-inspector";
                    output = Some(args.next().ok_or("Falta el archivo PNG de salida")?);
                }
                "--time" => {
                    seconds = args.next().ok_or("Falta el tiempo")?.parse()?;
                    if !seconds.is_finite() || seconds < 0.0 {
                        return Err("Tiempo inválido".into());
                    }
                }
                "--ship" => {
                    let number: usize = args.next().ok_or("Falta el número de nave")?.parse()?;
                    if !(1..=3).contains(&number) {
                        return Err("La nave debe ser 1, 2 o 3".into());
                    }
                    selected = number - 1;
                }
                "--help" => {
                    println!(
                        "cargo run --release\ncargo run --release -- --render-menu salida.png [--time 4] [--ship 2]\ncargo run --release -- --render-inspector salida.png [--ship 3]"
                    );
                    return Ok(());
                }
                _ => return Err(format!("Argumento desconocido: {argument}").into()),
            }
        }
        let output = output.ok_or("Especifica --render-menu o --render-inspector")?;
        if inspector {
            ships[selected].camera = ui::preview_camera(selected, seconds);
            ui.inspector(&mut framebuffer, &ships[selected], RenderMode::Solid, false);
        } else {
            let mut emphasis = [0.0; 3];
            emphasis[selected] = 1.0;
            ui.menu(
                &mut framebuffer,
                &ships,
                &ui::MenuView {
                    selected,
                    seconds,
                    emphasis,
                    ..Default::default()
                },
            );
        }
        if let Some(parent) = Path::new(output)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        framebuffer.save_png(output)?;
        println!("Imagen guardada: {output}");
        return Ok(());
    }

    let mut window = Window::new(
        "Orbital | Hangar de naves",
        WIDTH,
        HEIGHT,
        WindowOptions {
            resize: true,
            scale_mode: ScaleMode::AspectRatioStretch,
            ..WindowOptions::default()
        },
    )?;
    window.set_target_fps(60);
    let mut screen = Screen::Menu;
    let mut selected = 0usize;
    let mut mode = RenderMode::Solid;
    let mut clock = AnimationClock::default();
    let mut previous = Instant::now();
    let mut mouse_was_down = false;
    let mut emphasis = [1.0, 0.0, 0.0];

    while window.is_open() {
        let now = Instant::now();
        let dt = (now - previous).as_secs_f32().min(0.1);
        previous = now;
        if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
            if screen == Screen::Menu {
                break;
            }
            screen = Screen::Menu;
        }
        for (index, key) in [Key::Key1, Key::Key2, Key::Key3].into_iter().enumerate() {
            if window.is_key_pressed(key, KeyRepeat::No) {
                selected = index;
            }
        }
        let mouse = window
            .get_unscaled_mouse_pos(MouseMode::Discard)
            .and_then(|p| ui::canvas_mouse(p, window.get_size()));
        let mouse_down = window.get_mouse_down(MouseButton::Left);
        let clicked = mouse_down && !mouse_was_down;
        mouse_was_down = mouse_down;
        let hover = mouse.and_then(|p| (0..3).find(|&index| ui::card(index).contains(p)));
        let hover_button = mouse.is_some_and(|p| ui::LAUNCH.contains(p));
        let hover_pause = mouse.is_some_and(|p| ui::PAUSE.contains(p));
        let hover_back = mouse.is_some_and(|p| ui::BACK.contains(p));

        if screen == Screen::Menu {
            if window.is_key_pressed(Key::Left, KeyRepeat::Yes) {
                selected = (selected + 2) % 3;
            }
            if window.is_key_pressed(Key::Right, KeyRepeat::Yes) {
                selected = (selected + 1) % 3;
            }
            if window.is_key_pressed(Key::Tab, KeyRepeat::No) {
                let step =
                    if window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift) {
                        2
                    } else {
                        1
                    };
                selected = (selected + step) % 3;
            }
            if clicked && let Some(index) = hover {
                selected = index;
            }
            if window.is_key_pressed(Key::Space, KeyRepeat::No) || (clicked && hover_pause) {
                clock.paused = !clock.paused;
            }
            if window.is_active() {
                clock.advance(dt);
            }
            for (index, value) in emphasis.iter_mut().enumerate() {
                let target = if selected == index {
                    1.0
                } else if hover == Some(index) {
                    0.45
                } else {
                    0.0
                };
                *value += (target - *value) * (1.0 - (-dt * 12.0).exp());
            }
            if window.is_key_pressed(Key::Enter, KeyRepeat::No) || (clicked && hover_button) {
                ships[selected].camera = ui::preview_camera(selected, clock.seconds);
                screen = Screen::Inspector;
            }
        } else {
            if clicked && hover_back {
                screen = Screen::Menu;
            }
            let camera = &mut ships[selected].camera;
            let held = |a, b| window.is_key_down(a) || window.is_key_down(b);
            let yaw = i32::from(held(Key::Right, Key::D)) - i32::from(held(Key::Left, Key::A));
            let pitch = i32::from(held(Key::Up, Key::W)) - i32::from(held(Key::Down, Key::S));
            camera.orbit(yaw as f32 * dt * 1.7, pitch as f32 * dt * 1.7);
            let zoom =
                i32::from(window.is_key_down(Key::E)) - i32::from(window.is_key_down(Key::Q));
            camera.zoom = (camera.zoom * (zoom as f32 * dt * 1.2).exp()).clamp(0.45, 3.0);
            if window.is_key_pressed(Key::R, KeyRepeat::No) {
                *camera = Camera::new();
            }
            if window.is_key_pressed(Key::M, KeyRepeat::No) {
                mode = mode.next();
            }
        }

        let actionable = if screen == Screen::Menu {
            hover.is_some() || hover_button || hover_pause
        } else {
            hover_back
        };
        window.set_cursor_style(if actionable {
            CursorStyle::OpenHand
        } else {
            CursorStyle::Arrow
        });
        match screen {
            Screen::Menu => ui.menu(
                &mut framebuffer,
                &ships,
                &ui::MenuView {
                    selected,
                    seconds: clock.seconds,
                    paused: clock.paused,
                    hover,
                    hover_button,
                    hover_pause,
                    emphasis,
                },
            ),
            Screen::Inspector => ui.inspector(&mut framebuffer, &ships[selected], mode, hover_back),
        }
        if window.is_key_pressed(Key::P, KeyRepeat::No) {
            framebuffer.save_png("render.png")?;
        }
        window.update_with_buffer(&framebuffer.pixels, WIDTH, HEIGHT)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paused_previews_preserve_their_angle_and_resume_without_jumping() {
        let mut clock = AnimationClock::default();
        clock.advance(2.0);
        clock.paused = true;
        clock.advance(8.0);
        assert_eq!(clock.seconds, 2.0);
        clock.paused = false;
        clock.advance(0.5);
        assert_eq!(clock.seconds, 2.5);
    }
}
