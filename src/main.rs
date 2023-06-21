
#![windows_subsystem = "windows"]

// ------------------------------------------------------------

mod ogl;
mod painters;
mod picture;
mod display;
mod filepaths;
mod app;

// ------------------------------------------------------------

use winit::{event::*, event_loop::*};

// ------------------------------------------------------------

fn show_error_box<E>(error: &E, exit: bool) -> ()
where E: std::fmt::Display
{
    eprintln!("{error}");
    msgbox::create
    (
        "",
        &error.to_string(),
        msgbox::IconType::Error
    ).unwrap();
    if exit {std::process::exit(1)}
}

// ------------------------------------------------------------

fn main() -> !
{
    let path = std::env::args().nth(1).unwrap_or_default();
    let (mut app, event_loop) = app::App::new(path)
        .map_err(|e| show_error_box(&e, true))
        .unwrap();
    event_loop.run
    (
        move |event, _, control_flow| match event
        {
            Event::WindowEvent{event: window_event, ..} 
                => match window_event
            {
                WindowEvent::MouseInput
                {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                } => match app.drag_window()
                    .map_err(|e| show_error_box(&e, false))
                {
                    _ => {}
                }
                WindowEvent::KeyboardInput
                {
                    input: KeyboardInput
                    {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(keycode),
                        ..
                    },
                    ..
                } => match keycode
                {
                    VirtualKeyCode::Escape => 
                        *control_flow = ControlFlow::Exit,
                    VirtualKeyCode::Left => app.navigate(-1)
                        .map_err(|e| show_error_box(&e, true))
                        .unwrap(),
                    VirtualKeyCode::Right => app.navigate(1)
                        .map_err(|e| show_error_box(&e, true))
                        .unwrap(),
                    _ => {}
                }
                WindowEvent::DroppedFile(path) =>
                    app.change_path(path),
                WindowEvent::ScaleFactorChanged{scale_factor, ..} =>
                    app.set_scale_factor(scale_factor),
                _ => {}
            }
            Event::MainEventsCleared => app.refresh()
                .map_err(|e| show_error_box(&e, true))
                .unwrap(),
            Event::RedrawRequested(..) => app.draw()
                .map_err(|e| show_error_box(&e, true))
                .unwrap(),
            _ => {}
        }
    )
}
