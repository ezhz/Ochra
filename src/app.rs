
use std::{time::*, path::*};
use std::sync::{Arc, Mutex, mpsc::*};
use winit::{event::*, event_loop::*, dpi::*};
use super::{picture::{self, *}, renderer::*, filepaths::*};
use super::utility::*;
use super::painters::GLViewport;

// ----------------------------------------------------------------------------------------------------

const REFRESH_RATE: Duration = Duration::from_millis(125);

// ----------------------------------------------------------------------------------------------------

type ScreenSpacePosition<T> = PhysicalPosition<T>;

// ----------------------------------------------------------------------------------------------------

#[derive(Debug)]
struct WindowZoomInteraction
{
    cursor_captured: ScreenSpacePosition<f64>,
    window_origin_captured: ScreenSpacePosition<i32>,
    window_size_captured: PhysicalSize<u32>,
    screen_size_captured: PhysicalSize<u32>
}

impl WindowZoomInteraction
{
    fn new
    (
        window: &RenderWindow,
        cursor: ScreenSpacePosition<f64>
    ) -> anyhow::Result<Self>
    {
        let this = Self 
        {
            cursor_captured: cursor,
            window_origin_captured: window.get_position()?,
            window_size_captured: window.get_size(),
            screen_size_captured: window.get_screen_size()?
        };
        Ok(this)
    }

    pub fn compute_viewport
    (
        &self,
        cursor: ScreenSpacePosition<f64>
    ) -> GLViewport
    {
        let delta = vec!
        (
            (cursor.x - self.cursor_captured.x) *  0.003,
            (cursor.y - self.cursor_captured.y) * -0.003
        );
        let mut zoom = 1.0;
        for delta in delta
        {
            zoom *= match delta < 0.0
            {
                false => 1.0 + delta,
                true => 1.0 / (1.0 - delta)
            };
        };
        let mut origin =
        [
            (
                (self.window_origin_captured.x as f64 - self.cursor_captured.x)
                    * zoom + self.cursor_captured.x
            ).round() as i32,
            (
                (self.window_origin_captured.y as f64 - self.cursor_captured.y)
                    * zoom + self.cursor_captured.y
            ).round() as i32
        ];
        let size =
        [
            (self.window_size_captured.width as f64 * zoom)
                .round() as u32,
            (self.window_size_captured.height as f64 * zoom)
                .round() as u32
        ];
        origin[1] = self.screen_size_captured.height as i32 -
            (origin[1] + size[1] as i32);
        GLViewport{origin, size}
    }
}

// ----------------------------------------------------------------------------------------------------

enum WindowInteraction
{
    None,
    Unavailable,
    Drag,
    Zoom(WindowZoomInteraction)
}

impl WindowInteraction
{
    fn assert_unavailable(&self) -> ()
    {
        if let Self::Unavailable = self
        {
            return ()
        }
        panic!()
    }

    fn unavailable_to_none(&mut self) -> ()
    {
        if let Self::Unavailable = self
        {
            *self = Self::None
        }
    }
}

// ----------------------------------------------------------------------------------------------------

struct PictureLoader
{
    send_to_thread_path: Sender<std::path::PathBuf>,
    receive_on_main_path: Receiver<std::path::PathBuf>,
    send_to_thread_continue: Sender<()>,
    picture_result: Arc<Mutex<Option<PictureResult<Picture>>>>
}

impl PictureLoader
{
    fn new() -> Self 
    {
        let (send_to_thread_path, receive_on_thread_path):
            (Sender<std::path::PathBuf>, _) = channel();
        let (send_to_main_path, receive_on_main_path):
            (Sender<std::path::PathBuf>, _) = channel();
        let (send_to_thread_continue, receive_on_thread_continue):
            (Sender<()>, _) = channel();
        let picture_result = Arc::new(Mutex::new(None));
        let picture_result_thread = picture_result.clone();
        std::thread::spawn
        (
            move || loop
            {
                match receive_on_thread_path.try_iter().last()
                {
                    Some(filepath) =>
                    {
                        *picture_result_thread.lock().unwrap()
                            = Some(picture::open(&filepath));
                        send_to_main_path.send(filepath).unwrap();
                        receive_on_thread_continue.recv().unwrap()
                    }
                    None => {}
                }
            }
        );
        Self
        {
            send_to_thread_path,
            receive_on_main_path,
            send_to_thread_continue,
            picture_result
        }
    }
}

// ----------------------------------------------------------------------------------------------------

enum AppState
{
    Init(PathBuf),
    Idle(LiveNavigator, Instant),
    Loading(LiveNavigator),
    Drawing(LiveNavigator, Picture),
    Disabled
}

// ----------------------------------------------------------------------------------------------------

pub struct App
{
    window: RenderWindow,
    interaction: Option<WindowInteraction>,
    state: Option<AppState>,
    cursor: PhysicalPosition<f64>,
    loader: PictureLoader
}

impl App
{
    pub fn new<P: AsRef<Path>>(path: P) -> anyhow::Result
    <(
        Self,
        winit::event_loop::EventLoop<()>
    )>
    {
        let event_loop = EventLoop::new();
        let mut this = Self
        {
            window: RenderWindow::new(&event_loop)?,
            interaction: Some(WindowInteraction::Unavailable),
            state: Some(AppState::Init(path.as_ref().to_owned())),
            cursor: Default::default(),
            loader: PictureLoader::new(),
        };
        let scale_factor = this.window.get_scale_factor();
        this.window.set_scale_factor(scale_factor);
        Ok((this, event_loop))
    }

    fn end_zoom(&mut self, zoom: WindowZoomInteraction) -> ()
    {
        let mut viewport = self.window.get_viewport().clone();
        let window_origin = PhysicalPosition
        {
            x: viewport.origin[0],
            y: zoom.screen_size_captured.height as i32 -
            (
                viewport.origin[1] +
                viewport.size[1] as i32
            )
        };
        let window_size: PhysicalSize<u32> = viewport.size.into();
        viewport.origin = [0; 2];
        self.window.set_size(window_size);
        self.window.set_position(window_origin);
        self.window.set_viewport(viewport);
        self.interaction = Some(WindowInteraction::Unavailable);
        self.window.draw()
    }

    pub fn process_window_event
    (
        &mut self,
        event: WindowEvent,
        control_flow: &mut ControlFlow
    ) -> ()
    {
        match event
        {
            WindowEvent::CursorMoved{position, ..} =>
            {
                self.cursor = position;
                match &self.interaction.as_ref().unwrap()
                {
                    WindowInteraction::None => {}
                    WindowInteraction::Unavailable => {}
                    WindowInteraction::Drag => {}
                    WindowInteraction::Zoom(zoom) =>
                    {
                        let window_origin = self.window.get_position()
                            .map_err(|e| show_error_box(&e, true))
                            .unwrap();
                        let mut cursor = self.cursor;
                        cursor.x += window_origin.x as f64;
                        cursor.y += window_origin.y as f64;
                        let viewport = zoom.compute_viewport(cursor);
                        self.window.set_viewport(viewport);
                        self.window.draw()
                    }
                }
            }
            WindowEvent::MouseInput{state, button, ..} => match state
            {
                ElementState::Pressed => match button
                {
                    MouseButton::Left => match self.interaction.as_ref().unwrap()
                    {
                        WindowInteraction::None => match self.window.drag()
                            .map_err(|e| show_error_box(&e, false))
                        {
                            Ok(()) => self.interaction = Some(WindowInteraction::Drag),
                            Err(()) => {}
                        }
                        WindowInteraction::Unavailable => {}
                        WindowInteraction::Drag => unreachable!(),
                        WindowInteraction::Zoom(..) => {}
                    }
                    MouseButton::Right => match self.interaction.as_ref().unwrap()
                    {
                        WindowInteraction::None if !self.window.is_error() =>
                        {
                            let mut cursor = self.cursor;
                            let window_origin = self.window.get_position()
                                .map_err(|e| show_error_box(&e, true))
                                .unwrap();
                            cursor.x += window_origin.x as f64;
                            cursor.y += window_origin.y as f64;
                            let zoom = WindowZoomInteraction::new(&self.window, cursor)
                                .map_err(|e| show_error_box(&e, true))
                                .unwrap();
                            let viewport = GLViewport
                            {
                                origin:
                                [
                                    window_origin.x,
                                    zoom.screen_size_captured.height as i32 -
                                    (
                                        window_origin.y +
                                        zoom.window_size_captured.height as i32
                                    )
                                ],
                                size: zoom.window_size_captured.into()
                            };
                            self.window.set_viewport(viewport);
                            self.window.clear();
                            self.window.set_position(PhysicalPosition{x: 0, y: 0});
                            self.window.set_size(zoom.screen_size_captured);
                            self.draw();
                            self.interaction = Some(WindowInteraction::Zoom(zoom))
                        }
                        WindowInteraction::None => {}
                        WindowInteraction::Unavailable => {}
                        WindowInteraction::Drag => {}
                        WindowInteraction::Zoom(..) => unreachable!()
                    }
                    _ => {}
                }
                ElementState::Released => match button
                {
                    MouseButton::Left => match self.interaction.as_ref().unwrap()
                    {
                        WindowInteraction::Drag => self.interaction
                            = Some(WindowInteraction::Unavailable),
                        _ => {}
                    }
                    MouseButton::Right => self.interaction = 
                        match self.interaction.take().unwrap()
                    {
                        WindowInteraction::Zoom(zoom) =>
                        {
                            self.end_zoom(zoom);
                            return ()
                        }
                        interaction @ _ => Some(interaction)
                    },
                    _ => {}
                }
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
                VirtualKeyCode::Escape => control_flow.set_exit(),
                VirtualKeyCode::Left | VirtualKeyCode::Right
                    => match self.interaction.as_ref().unwrap()
                {
                    WindowInteraction::None =>
                    {
                        let direction = match keycode
                        {
                            VirtualKeyCode::Left => -1,
                            VirtualKeyCode::Right => 1,
                            _ => unreachable!()
                        };
                        self.navigate(direction)
                            .map_err(|e| show_error_box(&e, true))
                            .unwrap();
                        self.interaction = Some(WindowInteraction::Unavailable)
                    }
                    _ => {}
                }
                _ => {}
            }
            WindowEvent::DroppedFile(path) => match self.interaction.as_ref().unwrap()
            {
                WindowInteraction::None =>
                {
                    self.change_path(path);
                    self.interaction = Some(WindowInteraction::Unavailable)
                }
                _ => {}
            }
            WindowEvent::ScaleFactorChanged{scale_factor, ..} =>
            {
                self.interaction = match self.interaction.take().unwrap()
                {
                    WindowInteraction::Zoom(zoom) => 
                    {
                        self.end_zoom(zoom);
                        return ()
                    }
                    interaction @ _ => Some(interaction)
                };
                self.window.set_scale_factor(scale_factor);
                self.window.draw()
            }
            _ => {}
        }
    }

    #[must_use]
    fn navigate(&mut self, direction: i64) -> anyhow::Result<()>
    {
        self.state = match self.state.take().unwrap()
        {
            AppState::Idle(mut entries, ..)
            | AppState::Drawing(mut entries, ..)
            | AppState::Loading(mut entries, ..) =>
            {
                entries.navigate(direction);
                Some(self.show_blank(entries)?)
            }
            state @ _ => Some(state)
        };
        Ok(())
    }

    fn change_path<P: AsRef<Path>>(&mut self, path: P) -> ()
    {
        self.state = Some(AppState::Init(path.as_ref().to_owned()))
    }

    fn position_size_next
    (
        &mut self,
        targe_size: PhysicalSize<u32>
    ) -> anyhow::Result<()>
    {
        let previous_center = self.window.get_center()?;
        self.window.set_size(targe_size);
        let screen = self.window.get_screen_size()?;
        let screen = (screen.width as f32, screen.height as f32);
        let window = self.window.get_size();
        let window = (window.width as f32, window.height as f32);
        let mut fitted = window;
        let scale = 0.8;
        if window.0 > screen.0 * scale || window.1 > screen.1 * scale
        {
            let screen_ratio = screen.0 / screen.1;
            let window_ratio = window.0 / window.1;
            fitted = match screen_ratio > window_ratio
            {
                true => (window.0 * screen.1 / window.1, screen.1),
                false => (screen.0, window.1 * screen.0 / window.0)
            };
            fitted = (fitted.0 * scale, fitted.1 * scale);    
        }
        self.window.set_size(PhysicalSize::<f32>::from(fitted));
        let mut position = self.window.get_position()?;
        let new_center = self.window.get_center()?;
        position.x -= new_center.x - previous_center.x;
        position.y -= new_center.y - previous_center.y;
        self.window.set_position(position);
        self.window.set_viewport
        (
            GLViewport
            {
                origin: [0; 2],
                size: [fitted.0 as _, fitted.1 as _]
            }
        );
        Ok(())
    }

    #[must_use]
    fn show_blank(&mut self, entries: LiveNavigator) -> anyhow::Result<AppState>
    {
        match picture::read_dimensions(entries.selected())
        {
            Ok((width, height)) =>
            {
                self.window.use_blank_mode();
                self.position_size_next(PhysicalSize{width, height})?;
                self.loader.send_to_thread_path
                    .send(entries.selected().to_owned())
                    .unwrap();
                Ok(AppState::Loading(entries))
            }
            Err(error) =>
            {
                self.show_error(&error)?;
                Ok(AppState::Idle(entries, Instant::now()))
            }
        }
    }

    fn show_error<E>(&mut self, error: &E) -> anyhow::Result<()>
    where E: std::error::Error
    {
        self.window.use_error_mode(&error);
        let error_size = self.window.get_error_box_size();
        self.position_size_next(error_size) // **
    }

    #[must_use]
    pub fn refresh(&mut self) -> anyhow::Result<()>
    {
        let state = match self.state.take().unwrap()
        {
            AppState::Init(path) =>
            {
                self.interaction.as_ref().unwrap().assert_unavailable();
                self.window.set_visible(true);
                match LiveNavigator::from_path(&path, &picture::extensions())
                {
                    Ok(entries) => self.show_blank(entries)?,
                    Err(error) =>
                    {
                        self.show_error(&error)?;
                        AppState::Disabled
                    }
                }
            }
            AppState::Loading(entries) =>
            {
                match self.loader.receive_on_main_path.try_recv()
                {
                    Ok(path) => match path.eq(entries.selected())
                    {
                        true =>
                        {
                            let result = self.loader.picture_result
                                .lock().unwrap().take().unwrap();
                            let state = match result
                            {
                                Ok(picture) => AppState::Drawing(entries, picture),
                                Err(error) =>
                                {
                                    match self.interaction.take().unwrap()
                                    {
                                        WindowInteraction::Zoom(zoom) => self.end_zoom(zoom),
                                        interaction @ _ => self.interaction
                                            = Some(interaction)
                                    };
                                    self.show_error(&error)?;
                                    AppState::Idle(entries, Instant::now())
                                }
                            };
                            self.loader.send_to_thread_continue
                                .send(()).unwrap();
                            state
                        }
                        false =>
                        {
                            self.interaction.as_mut().unwrap().unavailable_to_none();
                            self.loader.send_to_thread_continue.send(()).unwrap();
                            AppState::Loading(entries)
                        }
                    }
                    Err(TryRecvError::Empty) =>
                    {
                        self.interaction.as_mut().unwrap().unavailable_to_none();
                        AppState::Loading(entries)
                    }
                    Err(error @ TryRecvError::Disconnected) =>
                    {
                        show_error_box(&error, true);
                        unreachable!()
                    }
                }
            }
            AppState::Drawing(entries, mut picture) => match picture
            {
                Picture::Still(mut still) =>
                {
                    match still.transform_to_icc(self.window.get_monitor_icc())
                    {
                        Ok(()) =>
                        {
                            self.interaction.as_mut().unwrap().unavailable_to_none();
                            self.window.use_picture_mode(&still);
                            self.window.draw()
                        }
                        Err(error) =>
                        {
                            match self.interaction.take().unwrap()
                            {
                                WindowInteraction::Zoom(zoom) => self.end_zoom(zoom),
                                interaction @ _ => self.interaction
                                    = Some(interaction)
                            };
                            self.show_error(&error)?
                        }
                    }
                    AppState::Idle(entries, Instant::now())
                }                
                Picture::Motion(ref mut player) =>
                {
                    if let Some(still) = player.next()
                    {
                        match still.clone()
                        {
                            Ok(mut still) => match still.transform_to_icc
                            (
                                self.window.get_monitor_icc()
                            )
                            {
                                Ok(()) =>
                                {
                                    self.interaction.as_mut().unwrap().unavailable_to_none();
                                    self.window.use_picture_mode(&still);
                                    self.window.draw()
                                }
                                Err(error) =>
                                {
                                    match self.interaction.take().unwrap()
                                    {
                                        WindowInteraction::Zoom(zoom) => self.end_zoom(zoom),
                                        interaction @ _ => self.interaction
                                            = Some(interaction)
                                    };
                                    self.show_error(&error)?
                                }
                            }
                            Err(error) =>
                            {
                                match self.interaction.take().unwrap()
                                {
                                    WindowInteraction::Zoom(zoom) => self.end_zoom(zoom),
                                    interaction @ _ => self.interaction
                                        = Some(interaction)
                                };
                                self.show_error(&error)?
                            }
                        }
                    }
                    AppState::Drawing(entries, picture)
                }
            }
            AppState::Idle(entries, timer) if timer.elapsed() > REFRESH_RATE =>
            {
                match entries.refresh()
                {
                    Ok((entries, dirty)) => match dirty
                    {
                        true =>
                        {
                            match self.interaction.take().unwrap()
                            {
                                WindowInteraction::Zoom(zoom) => self.end_zoom(zoom),
                                interaction @ _ => self.interaction
                                    = Some(interaction)
                            };
                            self.show_blank(entries)?
                        }
                        false =>
                        {
                            self.interaction.as_mut().unwrap().unavailable_to_none();
                            AppState::Idle(entries, Instant::now())
                        }
                    }
                    Err(error) =>
                    {
                        match self.interaction.take().unwrap()
                        {
                            WindowInteraction::Zoom(zoom) => self.end_zoom(zoom),
                            interaction @ _ => self.interaction
                                = Some(interaction)
                        };
                        self.show_error(&error)?;
                        AppState::Disabled
                    }
                }
            }
            AppState::Idle(entries, timer) =>
            {
                self.interaction.as_mut().unwrap().unavailable_to_none();
                AppState::Idle(entries, timer)
            } 
            state @ _  => state
        };
        Ok(self.state = Some(state))
    }
    
    pub fn draw(&mut self) -> ()
    {
        self.window.draw()
    }
}
