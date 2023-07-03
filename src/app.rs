
use
{
    std::path::*,
    winit::{event::*, event_loop::*},
    super::
    {
        loader::*,
        interface::*,
        reader::*
    }
};

// ----------------------------------------------------------------------------------------------------

pub struct App
{
    interface: Option<Interface>,
    reader: Option<PictureDirectoryReader> 
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
        let interface = Interface::new(&event_loop)?;
        let mut this = Self
        {
            interface: Some(interface),
            reader: None
        };
        this.reader = match PictureDirectoryReader::new(path)
        {
            Ok(reader) => Some(reader),
            Err(error) =>
            {
                this.show_error(&error)?;
                None
            }
        };
        Ok((this, event_loop))
    }

    pub fn process_window_event
    (
        &mut self,
        event: WindowEvent,
        control_flow: &mut ControlFlow
    ) -> anyhow::Result<()>
    {
        match event
        {
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
                {
                    self.disable_interaction()?;
                    Ok(control_flow.set_exit())
                }
                VirtualKeyCode::Left | VirtualKeyCode::Right
                    => match self.reader.take()
                {
                    Some(mut reader) =>
                    {
                        self.disable_interaction()?;
                        reader.navigate
                        (
                            match keycode
                            {
                                VirtualKeyCode::Left => -1,
                                VirtualKeyCode::Right => 1,
                                _ => unreachable!()
                            }
                        );
                        Ok(self.reader = Some(reader))
                    }
                    None => Ok(())
                }
                _ => Ok(())
            }
            WindowEvent::DroppedFile(path) => match self.reader.take()
            {
                Some(reader) => match reader.change_path(path)
                {
                    Ok(reader) => Ok(self.reader = Some(reader)),
                    Err(error) => Ok(self.show_error(&error)?)
                }
                None => match PictureDirectoryReader::new(path)
                {
                    Ok(reader) => Ok(self.reader = Some(reader)),
                    Err(error) => Ok(self.show_error(&error)?)
                }
            }
            WindowEvent::MouseInput
            {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } if self.interface.as_ref()
                .unwrap().is_error() =>
                Ok(()),
            _ =>
            {
                let interface = self.interface
                    .take().unwrap()
                    .refresh(&event)?;
                self.interface = Some(interface);
                Ok(())
            }
        }
    }

    fn disable_interaction(&mut self) -> anyhow::Result<()>
    {
        let interface = self.interface
            .take().unwrap()
            .disable_interaction()?;
        Ok(self.interface = Some(interface))
    }

    fn show_error<E>(&mut self, error: &E) -> anyhow::Result<()>
    where E: std::error::Error
    {
        let interface = self.interface
            .take().unwrap()
            .show_error(error)?;
        Ok(self.interface = Some(interface))
    }

    pub fn refresh(&mut self) -> anyhow::Result<()>
    {
        if let Some(reader) = self.reader.take()
        {
            match reader.refresh_filepaths()
            {
                Ok(mut reader) =>
                {
                    if let Some(result) = reader.next()
                    {
                        let mut interface = self.interface
                            .take().unwrap();
                        interface = match result
                        {
                            PictureLoadResult::PictureError(error)
                                => interface.show_error(&error)?,
                            PictureLoadResult::Loading(dimensions)
                                => interface.show_blank(dimensions)?,
                            PictureLoadResult::Loaded(still)
                                => interface.show_picture(still)?
                        };
                        self.interface = Some(interface)
                    }
                    self.reader = Some(reader)
                }
                Err(error) => self.show_error(&error)?
            }
        }
        Ok(())
    }

    pub fn draw(&mut self) -> ()
    {
        self.interface
            .as_mut().unwrap()
            .draw()
    }
}
