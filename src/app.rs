
use std::{time::*, path::*};
use super::{picture::{self, *}, display, filepaths::*};

// ----------------------------------------------------------------------------------------------------

const REFRESH_RATE: Duration = Duration::from_millis(125);

// ----------------------------------------------------------------------------------------------------

enum State
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
    display: display::Display,
    state: Option<State>
}

impl App
{
    pub fn new<P: AsRef<Path>>(path: P) -> (Self, winit::event_loop::EventLoop<()>)
    {
        let (display, event_loop) = display::Display::new();
        let state = State::Init(path.as_ref().to_owned());
        (Self{display, state: Some(state)}, event_loop)
    }
    
    pub fn change_path<P: AsRef<Path>>(&mut self, path: P) -> ()
    {
        self.state = Some(State::Init(path.as_ref().to_owned()))
    }

    fn load_picture(&mut self, entries: LiveNavigator) -> State
    {
        match picture::open(entries.selected())
        {
            Ok(picture) => State::Drawing(entries, picture),
            Err(error) =>
            {
                self.display.show_x(&error);
                State::Idle(entries, Instant::now())
            }
        }
    }
        
    pub fn navigate(&mut self, direction: i64) -> ()
    {        
        self.state = match self.state.take().unwrap()
        {
            State::Idle(mut entries, ..)
            | State::Drawing(mut entries, ..)
                =>
            {
                entries.navigate(direction);
                Some(State::Loading(entries))
            }
            state @ _ => Some(state)
        }
    }

    pub fn refresh(&mut self) -> ()
    {
        let state = match self.state.take().unwrap()
        {
            State::Init(path) =>
            {
                self.display.visible(true);
                match LiveNavigator::from_path(&path, &picture::extensions())
                {
                    Ok(entries) => self.load_picture(entries),
                    Err(error) => 
                    {
                        self.display.show_x(&error);
                        State::Disabled
                    }
                }
            }
            State::Loading(entries) => self.load_picture(entries),
            State::Drawing(entries, mut picture) => match picture
            {
                Picture::Still(result) =>
                {
                    match result
                    {
                        Ok(mut still) => match still.apply_icc_transform(self.display.get_icc())
                        {
                            Ok(()) => self.display.show_picture(&still),
                            Err(error) => self.display.show_x(&error)
                        }
                        Err(error) => self.display.show_x(&error)
                    }
                    State::Idle(entries, Instant::now())
                }                
                Picture::Motion(ref mut streamer) => match streamer.next()
                {
                    Ok(frame) =>
                    {
                        if let Some(still) = frame
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
                        State::Drawing(entries, picture)
                    }
                    Err(error) => 
                    {
                        self.display.show_x(error);
                        State::Idle(entries, Instant::now())
                    }
                }
            }
            State::Idle(entries, timer) if timer.elapsed() > REFRESH_RATE =>
            {
                match entries.refresh()
                {
                    Ok((entries, dirty)) => match dirty
                    {
                        false => State::Idle(entries, Instant::now()),
                        true => State::Loading(entries)
                    }
                    Err(error) =>
                    {
                        self.display.show_x(&error);
                        State::Disabled
                    }
                }
            }
            state @ _  => state
        };
        self.state = Some(state)
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
