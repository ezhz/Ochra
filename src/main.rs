
#![windows_subsystem = "windows"]

// ------------------------------------------------------------

mod ogl;
mod painters;
mod picture;
mod vector;
mod quad;
mod display;
mod filepaths;
mod app;

// ------------------------------------------------------------

use winit::{event::*, event_loop::*};

// ------------------------------------------------------------

fn main() -> !
{
    let path = std::env::args().nth(1).unwrap_or_default();
    let (mut app, event_loop) = app::App::new(path);
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
                } => app.drag_window(),
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
                    VirtualKeyCode::Left => app.navigate(-1),
                    VirtualKeyCode::Right => app.navigate(1),
                    _ => {}
                }
                WindowEvent::DroppedFile(path) =>
                    app.change_path(path),
                WindowEvent::ScaleFactorChanged{..} =>
                    app.on_scale_factor_changed(),
                _ => {}
            }
            Event::MainEventsCleared => app.refresh(),
            Event::RedrawRequested(..) => app.draw(),
            _ => {}
        }
    )
}
