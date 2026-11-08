mod black_hole;
mod camera;
mod collision;
mod color;
mod deep_space;
mod dust;
mod explosion;
mod fleet;
mod flight;
mod framebuffer;
mod galaxy;
mod galaxy_art;
mod home;
mod lighting;
mod menus;
mod model;
mod nebula;
mod noise;
mod orbit;
mod raster_batch;
mod renderer;
mod shaders;
mod skybox;
mod space;
mod texture;
mod thrusters;
mod travel;
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
    Home,
    Menu,
    Galaxy,
    Travel,
    Inspector,
    Flight,
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

fn launch_flight(ui: &ui::Ui, flight: &mut flight::Flight, screen: &mut Screen) {
    *flight = ui.new_flight();
    *screen = Screen::Flight;
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut ships = fleet::load()?;
    let mut ui = ui::Ui::new()?;
    let mut framebuffer = Framebuffer::new(WIDTH, HEIGHT);
    // Deterministic exports let us inspect the CPU renderer without a window.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        let mut output = None;
        let mut seconds = 0.0f32;
        let mut selected = 0usize;
        let mut benchmark_frames = 0usize;
        let mut galaxy_index = 0usize;
        let mut details_open = false;
        let mut home_page = home::Page::Main;
        let mut export_screen = Screen::Menu;
        let mut landmark = false;
        let mut planet = false;
        let mut target = None;
        let mut near_black_hole = false;
        let mut near_planet = None;
        let mut mouse_target = (0.0f32, 0.0f32);
        let mut args = args.iter();
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--benchmark-frames" => {
                    benchmark_frames =
                        args.next().ok_or("Falta la cantidad de cuadros")?.parse()?;
                    if !(1..=10000).contains(&benchmark_frames) {
                        return Err("La medición admite de 1 a 10000 cuadros".into());
                    }
                }
                "--render-home" | "--render-menu" | "--render-galaxies" | "--render-travel"
                | "--render-inspector" | "--render-flight" | "--render-landmark"
                | "--render-planet" => {
                    landmark = argument == "--render-landmark";
                    planet = argument == "--render-planet";
                    export_screen = match argument.as_str() {
                        "--render-home" => Screen::Home,
                        "--render-flight" => Screen::Flight,
                        "--render-galaxies" => Screen::Galaxy,
                        "--render-travel" => Screen::Travel,
                        "--render-inspector" => Screen::Inspector,
                        _ => Screen::Menu,
                    };
                    output = Some(args.next().ok_or("Falta el archivo PNG de salida")?);
                }
                "--home-section" => {
                    home_page = match args
                        .next()
                        .ok_or("Falta la seccion del menu principal")?
                        .as_str()
                    {
                        "principal" => home::Page::Main,
                        "controles" => home::Page::Controls,
                        "instrucciones" => home::Page::Instructions,
                        _ => {
                            return Err(
                                "Seccion invalida: principal, controles o instrucciones".into()
                            );
                        }
                    };
                }
                "--details" => details_open = true,
                "--galaxy" => {
                    let number: usize = args.next().ok_or("Falta el numero de galaxia")?.parse()?;
                    if !(1..=galaxy::GALAXIES.len()).contains(&number) {
                        return Err("La galaxia debe ser 1, 2 o 3".into());
                    }
                    galaxy_index = number - 1;
                }
                "--time" => {
                    seconds = args.next().ok_or("Falta el tiempo")?.parse()?;
                    if !seconds.is_finite() || seconds < 0.0 {
                        return Err("Tiempo inválido".into());
                    }
                }
                "--target" => {
                    target = Some(args.next().ok_or("Falta el nombre del escenario")?.as_str());
                }
                "--near-black-hole" => near_black_hole = true,
                "--near-planet" => {
                    near_planet = Some(args.next().ok_or("Falta el nombre del planeta")?)
                }
                "--ship" => {
                    let number: usize = args.next().ok_or("Falta el número de nave")?.parse()?;
                    if !(1..=3).contains(&number) {
                        return Err("La nave debe ser 1, 2 o 3".into());
                    }
                    selected = number - 1;
                }
                "--mouse" => {
                    mouse_target.0 = args
                        .next()
                        .ok_or("Falta la dirección horizontal")?
                        .parse()?;
                    mouse_target.1 = args.next().ok_or("Falta la dirección vertical")?.parse()?;
                    if ![mouse_target.0, mouse_target.1]
                        .iter()
                        .all(|v| v.is_finite() && v.abs() <= 1.0)
                    {
                        return Err("La dirección del mouse debe estar entre -1 y 1".into());
                    }
                }
                "--help" => {
                    println!(
                        "Beyond the Interstellar\ncargo run --release -- --render-home salida.png [--home-section principal|controles|instrucciones]"
                    );
                    println!(
                        "cargo run --release -- --render-travel salida.png [--galaxy 2] [--time 1.4]\n--render-galaxies admite --details para mostrar la descripcion seleccionada."
                    );
                    println!(
                        "cargo run --release -- --render-galaxies salida.png [--galaxy 1-3] [--time 2]\nLas exportaciones de vuelo admiten --galaxy 1-3 y --benchmark-frames 120."
                    );
                    println!(
                        "cargo run --release\ncargo run --release -- --render-menu salida.png [--time 4] [--ship 2]\ncargo run --release -- --render-inspector salida.png [--ship 3]\ncargo run --release -- --render-flight salida.png [--ship 1] [--time 2] [--mouse 0.5 -0.3] [--near-black-hole | --near-planet boreal]\ncargo run --release -- --render-landmark salida.png [--target accretion]\ncargo run --release -- --render-planet salida.png [--target boreal] [--time 20]"
                    );
                    return Ok(());
                }
                _ => return Err(format!("Argumento desconocido: {argument}").into()),
            }
        }
        let output = output.ok_or(
            "Especifica --render-home, --render-menu, --render-galaxies, --render-travel, --render-inspector, --render-flight, --render-landmark o --render-planet",
        )?;
        ui.select_galaxy(galaxy_index);
        if benchmark_frames > 0 && export_screen != Screen::Flight {
            return Err("La medicion requiere --render-flight".into());
        }
        if export_screen == Screen::Home {
            home::draw(
                &ui,
                &mut framebuffer,
                &home::Home {
                    page: home_page,
                    ..Default::default()
                },
            );
        } else if planet {
            ui.planet_preview(&mut framebuffer, target.unwrap_or("boreal"), seconds)?;
        } else if landmark {
            ui.landmark_preview(&mut framebuffer, target.unwrap_or("accretion"))?;
        } else if export_screen == Screen::Flight {
            if seconds > 600.0 {
                return Err("La exportación de vuelo admite hasta 600 segundos".into());
            }
            let mut flight = ui.new_flight();
            if let Some(name) = near_planet {
                if near_black_hole {
                    return Err("Elige --near-planet o --near-black-hole".into());
                }
                ui.approach_planet(&mut flight, name)?;
            }
            if near_black_hole {
                flight.position =
                    flight.black_hole.position + nalgebra_glm::Vec3::new(0.0, 0.0, 250.0);
            }
            flight.move_with_mouse(Some(mouse_target));
            for frame in 0..(seconds * 60.0).ceil() as u32 {
                let dt = (seconds - frame as f32 / 60.0).min(1.0 / 60.0);
                // Flight exports simulate holding W for the requested duration.
                flight.advance(dt, (1.0, 0.0), false, false);
            }
            ui.flight(&mut framebuffer, &ships[selected], &flight, seconds);
            if benchmark_frames > 0 {
                let start = Instant::now();
                for _ in 0..benchmark_frames {
                    ui.flight(&mut framebuffer, &ships[selected], &flight, seconds);
                    std::hint::black_box(&framebuffer.pixels);
                }
                let milliseconds = start.elapsed().as_secs_f64() * 1000.0 / benchmark_frames as f64;
                println!(
                    "Render CPU: {milliseconds:.3} ms/cuadro, {:.1} FPS teóricos, {benchmark_frames} cuadros",
                    1000.0 / milliseconds
                );
            }
        } else if export_screen == Screen::Galaxy {
            ui.galaxy_menu(
                &mut framebuffer,
                &ships[selected],
                &galaxy::GalaxyView {
                    selected: galaxy_index,
                    seconds,
                    details_open,
                    ..Default::default()
                },
            );
        } else if export_screen == Screen::Travel {
            let mut arrival = Framebuffer::new(WIDTH, HEIGHT);
            ui.flight(&mut arrival, &ships[selected], &ui.new_flight(), 0.0);
            ui.travel(
                &mut framebuffer,
                &travel::Travel::at(galaxy_index, seconds),
                &arrival,
            );
        } else if export_screen == Screen::Inspector {
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
        home::GAME_TITLE,
        WIDTH,
        HEIGHT,
        WindowOptions {
            resize: true,
            scale_mode: ScaleMode::AspectRatioStretch,
            ..WindowOptions::default()
        },
    )?;
    window.set_target_fps(60);
    let mut atlas = galaxy::GalaxyView::default();
    let mut transit = travel::Travel::new(0);
    let mut arrival = Framebuffer::new(WIDTH, HEIGHT);
    let mut screen = Screen::Home;
    let mut home_menu = home::Home::default();
    let mut previous_mouse = None;
    let mut selected = 0usize;
    let mut mode = RenderMode::Solid;
    let mut flight = ui.new_flight();
    let mut clock = AnimationClock::default();
    let mut previous = Instant::now();
    let mut mouse_was_down = false;
    let mut emphasis = [1.0, 0.0, 0.0];

    while window.is_open() {
        let now = Instant::now();
        let dt = (now - previous).as_secs_f32().min(0.1);
        previous = now;
        if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
            match screen {
                Screen::Home => home_menu.back(),
                Screen::Menu => screen = Screen::Home,
                Screen::Galaxy => screen = Screen::Menu,
                Screen::Travel => screen = Screen::Galaxy,
                Screen::Flight => screen = Screen::Galaxy,
                Screen::Inspector => {
                    flight.reset_steering();
                    screen = Screen::Flight;
                }
            }
        }
        for (index, key) in [Key::Key1, Key::Key2, Key::Key3].into_iter().enumerate() {
            if window.is_key_pressed(key, KeyRepeat::No) {
                if screen == Screen::Galaxy {
                    atlas.select(index);
                } else if matches!(screen, Screen::Menu | Screen::Flight | Screen::Inspector) {
                    selected = index;
                }
            }
        }
        let mouse = window
            .get_unscaled_mouse_pos(MouseMode::Discard)
            .and_then(|p| ui::canvas_mouse(p, window.get_size()));
        let mouse_moved = mouse != previous_mouse;
        previous_mouse = mouse;
        let home_hover = mouse.and_then(|point| home_menu.hit(point));
        let hover_home = mouse.is_some_and(|point| home::HANGAR_BACK.contains(point));
        let mouse_down = window.get_mouse_down(MouseButton::Left);
        let clicked = mouse_down && !mouse_was_down;
        mouse_was_down = mouse_down;
        let hover = mouse.and_then(|p| (0..3).find(|&index| ui::card(index).contains(p)));
        let galaxy_hover = mouse.and_then(|point| atlas.hit(point));
        let hover_button = mouse.is_some_and(|p| ui::LAUNCH.contains(p))
            && (screen != Screen::Galaxy || atlas.details_open);
        let hover_pause = mouse.is_some_and(|p| ui::PAUSE.contains(p));
        let hover_back = mouse.is_some_and(|p| {
            if screen == Screen::Galaxy {
                menus::DESTINATION_BACK.contains(p)
            } else {
                ui::BACK.contains(p)
            }
        });
        let hover_close = screen == Screen::Galaxy
            && atlas.details_open
            && mouse.is_some_and(|p| menus::DESTINATION_CLOSE.contains(p));

        if screen == Screen::Home {
            if home_menu.page == home::Page::Main {
                let backwards = window.is_key_pressed(Key::Up, KeyRepeat::Yes)
                    || window.is_key_pressed(Key::W, KeyRepeat::Yes);
                let forwards = window.is_key_pressed(Key::Down, KeyRepeat::Yes)
                    || window.is_key_pressed(Key::S, KeyRepeat::Yes);
                if backwards {
                    home_menu.navigate(-1);
                }
                if forwards {
                    home_menu.navigate(1);
                }
                if window.is_key_pressed(Key::Tab, KeyRepeat::No) {
                    let shift =
                        window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
                    home_menu.navigate(if shift { -1 } else { 1 });
                }
                if (mouse_moved || clicked)
                    && let Some(index) = home_hover
                {
                    home_menu.selected = index;
                }
            }
            if window.is_key_pressed(Key::Enter, KeyRepeat::No) || (clicked && home_hover.is_some())
            {
                match home_menu.activate() {
                    home::Action::Ships => screen = Screen::Menu,
                    home::Action::Destinations => {
                        atlas.details_open = false;
                        screen = Screen::Galaxy;
                    }
                    home::Action::Exit => break,
                    home::Action::None => {}
                }
            }
        } else if screen == Screen::Menu {
            if window.is_key_pressed(Key::Left, KeyRepeat::Yes)
                || window.is_key_pressed(Key::Up, KeyRepeat::Yes)
            {
                selected = (selected + 2) % 3;
            }
            if window.is_key_pressed(Key::Right, KeyRepeat::Yes)
                || window.is_key_pressed(Key::Down, KeyRepeat::Yes)
            {
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
                atlas.details_open = false;
                screen = Screen::Galaxy;
            }
            if clicked && hover_home {
                screen = Screen::Home;
            }
        } else if screen == Screen::Galaxy {
            if window.is_active() {
                atlas.advance(dt);
                let axis = |positive, negative| {
                    i32::from(window.is_key_down(positive))
                        - i32::from(window.is_key_down(negative))
                };
                atlas.pan.0 =
                    (atlas.pan.0 + axis(Key::D, Key::A) as f32 * dt * 180.0).clamp(-140.0, 140.0);
                atlas.pan.1 =
                    (atlas.pan.1 + axis(Key::S, Key::W) as f32 * dt * 180.0).clamp(-80.0, 80.0);
                let wheel = window.get_scroll_wheel().map_or(0.0, |(_, y)| y);
                let zoom_key = axis(Key::Equal, Key::Minus) as f32 * dt;
                atlas.zoom = (atlas.zoom * (wheel * 0.10 + zoom_key).exp()).clamp(0.65, 1.35);
            }
            if window.is_key_pressed(Key::R, KeyRepeat::No) {
                atlas = galaxy::GalaxyView {
                    selected: atlas.selected,
                    details_open: atlas.details_open,
                    seconds: atlas.seconds,
                    ..Default::default()
                };
            }
            let previous = window.is_key_pressed(Key::Left, KeyRepeat::Yes)
                || window.is_key_pressed(Key::Up, KeyRepeat::Yes);
            let next = window.is_key_pressed(Key::Right, KeyRepeat::Yes)
                || window.is_key_pressed(Key::Down, KeyRepeat::Yes);
            if previous {
                atlas.select((atlas.selected + 2) % 3);
            }
            if next || window.is_key_pressed(Key::Tab, KeyRepeat::No) {
                let backwards = !next
                    && (window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift));
                atlas.select((atlas.selected + if backwards { 2 } else { 1 }) % 3);
            }
            if clicked && let Some(index) = galaxy_hover {
                atlas.select(index);
            }
            if clicked
                && (hover_close
                    || (galaxy_hover.is_none()
                        && !hover_button
                        && !mouse.is_some_and(|p| menus::DESTINATION_PANEL.contains(p))))
            {
                atlas.details_open = false;
            }
            if clicked && hover_back {
                screen = Screen::Menu;
            }
            if window.is_key_pressed(Key::Enter, KeyRepeat::No) || (clicked && hover_button) {
                if atlas.details_open {
                    ui.select_galaxy(atlas.selected);
                    ui.flight(&mut arrival, &ships[selected], &ui.new_flight(), 0.0);
                    transit = travel::Travel::new(atlas.selected);
                    screen = Screen::Travel;
                } else {
                    atlas.select(atlas.selected);
                }
            }
        } else if screen == Screen::Travel {
            if window.is_active() && transit.advance(dt) {
                launch_flight(&ui, &mut flight, &mut screen);
            }
        } else if screen == Screen::Inspector {
            if clicked && hover_back {
                flight.reset_steering();
                screen = Screen::Flight;
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
        } else {
            if window.is_active() {
                flight.move_with_mouse(mouse.map(|(x, y)| {
                    (
                        x / (WIDTH as f32 * 0.5) - 1.0,
                        y / (HEIGHT as f32 * 0.5) - 1.0,
                    )
                }));
                let movement = (
                    i32::from(window.is_key_down(Key::W)) - i32::from(window.is_key_down(Key::S)),
                    i32::from(window.is_key_down(Key::D)) - i32::from(window.is_key_down(Key::A)),
                );
                flight.advance(
                    dt,
                    (movement.0 as f32, movement.1 as f32),
                    window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift),
                    window.is_key_down(Key::Space),
                );
                clock.advance(dt);
            } else {
                flight.reset_steering();
            }
            if window.is_key_pressed(Key::Key1, KeyRepeat::No) {
                selected = 0;
            } else if window.is_key_pressed(Key::Key2, KeyRepeat::No) {
                selected = 1;
            } else if window.is_key_pressed(Key::Key3, KeyRepeat::No) {
                selected = 2;
            }
            if window.is_key_pressed(Key::I, KeyRepeat::No) {
                ships[selected].camera = flight.camera;
                screen = Screen::Inspector;
            }
        }

        let actionable = if screen == Screen::Home {
            home_hover.is_some()
        } else if screen == Screen::Menu {
            hover.is_some() || hover_button || hover_pause || hover_home
        } else if screen == Screen::Galaxy {
            galaxy_hover.is_some() || hover_button || hover_back || hover_close
        } else if screen == Screen::Inspector {
            hover_back
        } else {
            false
        };
        window.set_cursor_visibility(screen != Screen::Travel);
        window.set_cursor_style(if screen == Screen::Flight {
            CursorStyle::Crosshair
        } else if actionable {
            CursorStyle::OpenHand
        } else {
            CursorStyle::Arrow
        });
        match screen {
            Screen::Home => home::draw(&ui, &mut framebuffer, &home_menu),
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
            Screen::Galaxy => ui.galaxy_menu(
                &mut framebuffer,
                &ships[selected],
                &galaxy::GalaxyView {
                    hover: galaxy_hover,
                    hover_button,
                    hover_back,
                    ..atlas
                },
            ),
            Screen::Travel => ui.travel(&mut framebuffer, &transit, &arrival),
            Screen::Inspector => ui.inspector(&mut framebuffer, &ships[selected], mode, hover_back),
            Screen::Flight => {
                ui.flight(&mut framebuffer, &ships[selected], &flight, flight.seconds)
            }
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
    fn hangar_launch_and_relaunch_keep_planet_collisions_and_impact_explosions() {
        let ui = ui::Ui::new().unwrap();
        let mut flight = ui.new_flight();
        let mut screen = Screen::Menu;
        for attempt in 0..2 {
            launch_flight(&ui, &mut flight, &mut screen);
            assert!(screen == Screen::Flight);
            ui.approach_planet(&mut flight, "Boreal").unwrap();
            for _ in 0..180 {
                flight.advance(1.0 / 60.0, (1.0, 0.0), true, false);
                if flight.crash.is_some() {
                    break;
                }
            }
            assert_eq!(
                flight.crash.as_ref().map(|crash| crash.planet),
                Some("Boreal"),
                "Launching from the hangar must preserve collisions on attempt {attempt}"
            );
            screen = Screen::Menu;
        }
    }
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
