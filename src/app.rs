
use std::{time::*, path::*};
use std::sync::{Arc, Mutex, mpsc::*};
use super::{picture::{self, *}, display, filepaths::*};

// ----------------------------------------------------------------------------------------------------

const REFRESH_RATE: Duration = Duration::from_millis(125);

// ----------------------------------------------------------------------------------------------------

enum PictureState
{
    Init(PathBuf),
    Idle(LiveNavigator, Instant),
    Loading(LiveNavigator),
    Drawing(LiveNavigator, Picture),
    Disabled
}

// ----------------------------------------------------------------------------------------------------

struct PictureLoader
{
    send_to_thread_path: Sender<std::path::PathBuf>,
    receive_on_main_path: Receiver<std::path::PathBuf>,
    send_to_thread_continue: Sender<()>,
    send_to_main_result: Arc<Mutex<Option<PictureResult<Picture>>>>
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
        let send_to_main_result = Arc::new(Mutex::new(None));
        let picture_result_thread = send_to_main_result.clone();
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
            send_to_main_result
        }
    }
}


// ----------------------------------------------------------------------------------------------------

pub struct App
{
    display: display::Display,
    state: Option<PictureState>,
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
        let (display, event_loop) = display::Display::new()?;
        Ok
        ((
            Self
            {
                display,
                state: Some(PictureState::Init(path.as_ref().to_owned())),
                loader: PictureLoader::new()
            }, 
            event_loop
        ))
    }
    
    pub fn change_path<P: AsRef<Path>>(&mut self, path: P) -> ()
    {
        self.state = Some(PictureState::Init(path.as_ref().to_owned()))
    }

    #[must_use]
    fn show_blank(&mut self, entries: LiveNavigator) -> anyhow::Result<PictureState>
    {
        match picture::read_dimensions(entries.selected())
        {
            Ok((width, height)) =>
            {
                self.display.show_blank(winit::dpi::PhysicalSize{width, height})?;
                self.loader.send_to_thread_path
                    .send(entries.selected().to_owned())
                    .unwrap();
                Ok(PictureState::Loading(entries))
            }
            Err(error) =>
            {
                self.display.show_error(&error)?;
                Ok(PictureState::Idle(entries, Instant::now()))
            }
        }
    }

    #[must_use]
    pub fn navigate(&mut self, direction: i64) -> anyhow::Result<()>
    {
        self.state = match self.state.take().unwrap()
        {
            PictureState::Idle(mut entries, ..)
            | PictureState::Drawing(mut entries, ..)
            | PictureState::Loading(mut entries, ..) =>
            {
                entries.navigate(direction);
                Some(self.show_blank(entries)?)
            }
            state @ _ => Some(state)
        };
        Ok(())
    }

    #[must_use]
    pub fn refresh(&mut self) -> anyhow::Result<()>
    {
        let state = match self.state.take().unwrap()
        {
            PictureState::Init(path) =>
            {
                self.display.set_visible(true);
                match LiveNavigator::from_path(&path, &picture::extensions())
                {
                    Ok(entries) => self.show_blank(entries)?,
                    Err(error) => 
                    {
                        self.display.show_error(&error)?;
                        PictureState::Disabled
                    }
                }
            }
            PictureState::Loading(entries) =>
            {
                match self.loader.receive_on_main_path.try_recv()
                {
                    Ok(path) => match path.eq(entries.selected())
                    {
                        true =>
                        {
                            let state = match self.loader.send_to_main_result
                                .lock().unwrap().take().unwrap()
                            {
                                Ok(picture) => PictureState::Drawing(entries, picture),
                                Err(error) =>
                                {
                                    self.display.show_error(&error)?;
                                    PictureState::Idle(entries, Instant::now())
                                }
                            };
                            self.loader.send_to_thread_continue.send(()).unwrap();
                            state
                        }
                        false =>
                        {
                            self.loader.send_to_thread_continue.send(()).unwrap();
                            PictureState::Loading(entries)
                        }
                    }
                    Err(TryRecvError::Empty) => PictureState::Loading(entries),
                    Err(error @ TryRecvError::Disconnected) =>
                    {
                        self.display.show_error(&error)?;
                        PictureState::Disabled
                    }
                }
            }
            PictureState::Drawing(entries, mut picture) => match picture
            {
                Picture::Still(mut still) =>
                {
                    match still.apply_icc_transform(self.display.get_icc())
                    {
                        Ok(()) => self.display.show_picture(&still)?,
                        Err(error) => self.display.show_error(&error)?
                    }
                    PictureState::Idle(entries, Instant::now())
                }                
                Picture::Motion(ref mut player) =>
                {
                    if let Some(still) = player.next()
                    {
                        match still.clone()
                        {
                            Ok(mut still) => match still.apply_icc_transform(self.display.get_icc())
                            {
                                Ok(()) => self.display.show_picture(&still)?,
                                Err(error) => self.display.show_error(&error)?
                            }
                            Err(error) => self.display.show_error(&error)?
                        }
                    }
                    PictureState::Drawing(entries, picture)
                }
            }
            PictureState::Idle(entries, timer) if timer.elapsed() > REFRESH_RATE =>
            {
                match entries.refresh()
                {
                    Ok((entries, dirty)) => match dirty
                    {
                        false => PictureState::Idle(entries, Instant::now()),
                        true => self.show_blank(entries)?
                    }
                    Err(error) =>
                    {
                        self.display.show_error(&error)?;
                        PictureState::Disabled
                    }
                }
            }
            state @ _  => state
        };
        Ok(self.state = Some(state))
    }

    pub fn set_scale_factor(&mut self, scale_factor: f64) -> ()
    {
        self.display.set_scale_factor(scale_factor)
    }

    #[must_use]
    pub fn drag_window(&self) -> anyhow::Result<()>
    {
        self.display.drag()
    }
    
    #[must_use]
    pub fn draw(&mut self) -> anyhow::Result<()>
    {
        self.display.draw()
    }
}
