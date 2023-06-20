
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
    pub fn new<P: AsRef<Path>>(path: P) -> (Self, winit::event_loop::EventLoop<()>)
    {
        let (display, event_loop) = display::Display::new();
        let state = PictureState::Init(path.as_ref().to_owned());
        (
            Self
            {
                display,
                state: Some(state),
                loader: PictureLoader::new()
            }, 
            event_loop
        )
    }
    
    pub fn change_path<P: AsRef<Path>>(&mut self, path: P) -> ()
    {
        self.state = Some(PictureState::Init(path.as_ref().to_owned()))
    }

    fn show_loader(&mut self, entries: LiveNavigator) -> PictureState
    {
        match picture::read_dimensions(entries.selected())
        {
            Ok((width, height)) =>
            {
                self.display.show_loader(winit::dpi::PhysicalSize{width, height});
                self.loader.send_to_thread_path
                    .send(entries.selected().to_owned())
                    .unwrap();
                PictureState::Loading(entries)
            }
            Err(error) =>
            {
                self.display.show_x(&error);
                PictureState::Idle(entries, Instant::now())
            }
        }
    }

    pub fn navigate(&mut self, direction: i64) -> ()
    {
        self.state = match self.state.take().unwrap()
        {
            PictureState::Idle(mut entries, ..)
            | PictureState::Drawing(mut entries, ..)
            | PictureState::Loading(mut entries, ..) =>
            {
                entries.navigate(direction);
                Some(self.show_loader(entries))
            }
            state @ _ => Some(state)
        }
    }

    pub fn refresh(&mut self) -> ()
    {
        let state = match self.state.take().unwrap()
        {
            PictureState::Init(path) =>
            {
                self.display.visible(true);
                match LiveNavigator::from_path(&path, &picture::extensions())
                {
                    Ok(entries) => self.show_loader(entries),
                    Err(error) => 
                    {
                        self.display.show_x(&error);
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
                                    self.display.show_x(&error);
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
                        self.display.show_x(&error);
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
                        Ok(()) => self.display.show_picture(&still),
                        Err(error) => self.display.show_x(&error)
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
                                Ok(()) => self.display.show_picture(&still),
                                Err(error) => self.display.show_x(&error)
                            }
                            Err(error) => self.display.show_x(&error)
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
                        true => self.show_loader(entries)
                    }
                    Err(error) =>
                    {
                        self.display.show_x(&error);
                        PictureState::Disabled
                    }
                }
            }
            state @ _  => state
        };
        self.state = Some(state)
    }
    
    pub fn on_scale_factor_changed(&mut self) -> ()
    {
        self.display.on_scale_factor_changed()
    }

    pub fn drag_window(&self) -> ()
    {
        self.display.drag()
    }
    
    pub fn draw(&mut self) -> ()
    {
        self.display.draw()
    }
}
