
#![windows_subsystem = "windows"]

// ------------------------------------------------------------

mod utility;
mod cases;
mod ogl;
mod painters;
mod picture;
mod loader;
mod reader;
mod renderer;
mod interface;
mod navigator;
mod app;

// ------------------------------------------------------------

fn main() -> !
{
    let path = std::env::args().nth(1).unwrap_or_default();
    let (mut app, event_loop) = app::App::new(path)
        .map_err(|e| utility::show_error_box(&e, true))
        .unwrap();
    event_loop.run
    (
        move |event, _, control_flow| match event
        {
            winit::event::Event::WindowEvent{event: window_event, ..}
                => app.process_window_event(window_event, control_flow),
            winit::event::Event::MainEventsCleared => app.refresh(),
            winit::event::Event::RedrawRequested(..) => Ok(app.draw()),
            _ => Ok(())
        }.map_err(|e| utility::show_error_box(&e, true))
            .unwrap()
    )
}
