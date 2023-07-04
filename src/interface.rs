
use
{
    std::{fmt, time::*},
    winit::{window::*, event::*, event_loop::*, dpi::*},
    super::
    {
        utility::*,
        cases::*,
        painters::*,
        picture::*,
        renderer::*
    }
};

// ------------------------------------------------------------

const MIN_WINDOW_SIZE: f64 = 100.0;
const SPIN_TIME: Duration = Duration::from_millis(10);

// ------------------------------------------------------------

type ScreenSpacePosition<T> = PhysicalPosition<T>;

// ------------------------------------------------------------

struct InterfaceRenderer
{
    main: RenderWindow,
    stamp: RenderWindow
}

impl InterfaceRenderer
{
    fn new
    (
        event_loop: &EventLoopWindowTarget<()>
    ) -> anyhow::Result<Self>
    {
        let mut main = RenderWindow::new(event_loop)?;
        let scale_factor = main.get_scale_factor();
        main.set_scale_factor(scale_factor);
        let mut stamp = RenderWindow::new(event_loop)?;
        let scale_factor = stamp.get_scale_factor();
        stamp.set_scale_factor(scale_factor);
        stamp.set_level(WindowLevel::AlwaysOnBottom);
        stamp.set_skip_taskbar(true);
        stamp.clear();
        spin(SPIN_TIME);
        stamp.set_visible(true);
        Ok(Self{main, stamp})
    }

    fn window_id(&self) -> WindowId
    {
        self.main.id()
    }

    fn position_size_next
    (
        &mut self,
        targe_size: PhysicalSize<u32>
    ) -> anyhow::Result<()>
    {
        let previous_center = self.main.get_center()?;
        self.set_window_size(targe_size);
        let screen = self.get_screen_size()?;
        let screen = [screen.width as f64, screen.height as f64];
        let window = self.get_window_size();
        let window = [window.width as f64, window.height as f64];
        let window_ratio = window[0] / window[1];
        let mut fitted = window;
        let scale = 0.8;
        if window[0] > screen[0] * scale || window[1] > screen[1] * scale
        {
            let screen_ratio = screen[0] / screen[1];
            fitted = match screen_ratio > window_ratio
            {
                true => [window[0] * screen[1] / window[1], screen[1]],
                false => [screen[0], window[1] * screen[0] / window[0]]
            };
            fitted = [fitted[0] * scale, fitted[1] * scale];
        }
        if fitted[0] < MIN_WINDOW_SIZE || fitted[1] < MIN_WINDOW_SIZE
        {
            let scale = match window_ratio > 1.0
            {
                true => MIN_WINDOW_SIZE / fitted[0],
                false => MIN_WINDOW_SIZE / fitted[1]
            };
            fitted[0] *= scale;
            fitted[1] *= scale
        }
        self.set_window_size(PhysicalSize::<f32>::from(fitted));
        let mut position = self.get_window_origin()?;
        let new_center = self.main.get_center()?;
        position.x -= new_center.x - previous_center.x;
        position.y -= new_center.y - previous_center.y;
        let viewport = GLViewport
        {
            origin: [0; 2],
            size: 
            [
                fitted[0] as _,
                fitted[1] as _
            ]
        };
        self.set_window_origin(position);
        self.set_viewport(&viewport);
        Ok(())
    }

    fn set_window_size<S: Into<Size>>(&mut self, size: S) -> ()
    {
        self.main.set_size(size);
    }

    fn get_window_size(&self) -> PhysicalSize<u32>
    {
        self.main.get_size()
    }

    fn get_window_origin(&self) -> Result
    <
        PhysicalPosition<i32>,
        winit::error::NotSupportedError
    >
    {
        self.main.get_origin()
    }

    fn set_window_origin<P: Into<Position>>(&self, origin: P) -> ()
    {
        self.main.set_origin(origin)
    }

    fn get_screen_size(&self) -> anyhow::Result<PhysicalSize<u32>>
    {
        self.main.get_screen_size()
    }

    fn get_viewport(&self) -> &GLViewport
    {
        self.main.get_viewport()
    }

    fn set_viewport(&mut self, viewport: &GLViewport) -> ()
    {
        self.main.set_viewport(viewport);
        self.stamp.set_viewport(viewport)
    }

    fn set_scale_factor(&mut self, scale_factor: f64) -> ()
    {
        self.main.set_scale_factor(scale_factor);
        self.stamp.set_scale_factor(scale_factor);
        self.draw()
    }

    fn show_blank
    (
        &mut self,
        dimensions: PictureDimensions
    ) -> anyhow::Result<()>
    {
        self.main.use_blank_mode();
        self.stamp.use_blank_mode();
        self.position_size_next(dimensions.into())
            .map(|_| self.draw())
    }

    fn show_picture(&mut self, mut still: StillPicture) -> PictureResult<()>
    {
        still.transform_to_icc(self.main.get_monitor_icc())?;
        self.main.use_picture_mode(&still);
        self.stamp.use_picture_mode(&still);
        Ok(self.draw())
    }

    fn show_error<E>(&mut self, error: &E) -> anyhow::Result<()>
    where E: std::error::Error
    {
        self.main.use_error_mode(&error);
        let error_size = self.main.get_error_box_size();
        self.position_size_next(error_size)
            .map(|_| self.draw())
    }

    fn is_error(&self) -> bool
    {
        self.main.is_error()
    }

    fn drag(&self) -> anyhow::Result<()>
    {
        self.main.drag()
    }

    fn clear(&self) -> ()
    {
        self.main.clear()
    }

    fn draw(&mut self) -> ()
    {
        self.main.set_visible(true);
        self.main.draw()
    }
}

// ------------------------------------------------------------

struct DisabledInteraction;

// ------------------------------------------------------------

struct NoInteraction;

// ------------------------------------------------------------

struct DragInteraction;

// ------------------------------------------------------------

struct ZoomInteraction
{
    cursor_captured: ScreenSpacePosition<f64>,
    window_origin_captured: ScreenSpacePosition<i32>,
    window_size_captured: PhysicalSize<u32>,
    screen_size_captured: PhysicalSize<u32>,
    zoom_bounds: [f64; 2]
}

impl ZoomInteraction
{
    const ZOOM_SPEED: f64 = 0.003;

    fn new
    (
        interface: &InterfaceRenderer,
        cursor: PhysicalPosition<f64>
    ) -> anyhow::Result<Self>
    {
        let mut this = Self
        {
            cursor_captured: Self::cursor_to_screen_space(interface, cursor)?,
            window_origin_captured: interface.get_window_origin()?,
            window_size_captured: interface.get_window_size(),
            screen_size_captured: interface.get_screen_size()?,
            zoom_bounds: [0.0; 2]
        };
        let screen_ratio = this.screen_size_captured.width as f64
            / this.screen_size_captured.height as f64;
        let window_ratio = this.window_size_captured.width as f64
            / this.window_size_captured.height as f64;
        this.zoom_bounds[0] = match window_ratio > 1.0
        {
            true => MIN_WINDOW_SIZE / this.window_size_captured.width as f64,
            false => MIN_WINDOW_SIZE / this.window_size_captured.height as f64
        };
        this.zoom_bounds[1] = match screen_ratio > window_ratio
        {
            true => this.screen_size_captured.height as f64
                / this.window_size_captured.height as f64,
            false => this.screen_size_captured.width as f64
                / this.window_size_captured.width as f64
        };
        Ok(this)
    }

    fn cursor_to_screen_space
    (
        interface: &InterfaceRenderer,
        mut cursor: PhysicalPosition<f64>
    ) -> anyhow::Result<ScreenSpacePosition<f64>>
    {
        let window_origin = interface.get_window_origin()?;
        cursor.x += window_origin.x as f64;
        cursor.y += window_origin.y as f64;
        Ok(cursor)
    }

    fn compute_viewport
    (
        &self,
        interface: &InterfaceRenderer,
        cursor: &PhysicalPosition<f64>
    ) -> anyhow::Result<GLViewport>
    {
        let cursor = Self::cursor_to_screen_space
        (
            interface,
            cursor.clone()
        )?;
        let delta = vec!
        (
            (cursor.x - self.cursor_captured.x) *  Self::ZOOM_SPEED,
            (cursor.y - self.cursor_captured.y) * -Self::ZOOM_SPEED
        );
        let mut zoom = 1.0;
        for delta in delta
        {
            zoom *= match delta < 0.0
            {
                true => 1.0 / (1.0 - delta),
                false => 1.0 + delta
            };
        };
        zoom = zoom.clamp(self.zoom_bounds[0], self.zoom_bounds[1]);
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
        Ok(GLViewport{origin, size})
    }
}

// ------------------------------------------------------------

struct InteractionMachine<I>
{
    interface: InterfaceRenderer,
    cursor: PhysicalPosition<f64>,
    interaction: I
}

impl<I> InteractionMachine<I>
{
    fn window_id(&self) -> WindowId
    {
        self.interface.window_id()
    }

    fn is_error(&self) -> bool
    {
        self.interface.is_error()
    }

    fn draw(&mut self) -> ()
    {
        self.interface.draw()
    }
}

impl InteractionMachine<DisabledInteraction>
{
    fn new(event_loop: &EventLoopWindowTarget<()>) -> anyhow::Result<Self>
    {
        InterfaceRenderer::new(event_loop).map
        (
            |window| Self
            {
                interface: window,
                cursor: Default::default(),
                interaction: DisabledInteraction
            }
        )
    }

    fn refresh(&mut self, event: &WindowEvent) -> ()
    {
        match *event
        {
            WindowEvent::ScaleFactorChanged{scale_factor, ..}
                => self.interface.set_scale_factor(scale_factor),
            WindowEvent::CursorMoved{position, ..} =>
                self.cursor = position,
            _ => {}
        }
    }

    fn show_blank
    (
        mut self,
        dimensions: PictureDimensions
    ) -> anyhow::Result<InteractionMachine<NoInteraction>>
    {
        self.interface.show_blank(dimensions)?;
        Ok(self.into())
    }

    fn show_picture(mut self, still: StillPicture)
        -> anyhow::Result<InteractionMachine<NoInteraction>>
    {
        match self.interface.show_picture(still)
        {
            Ok(()) => Ok(self.into()),
            Err(error) => self.show_error(&error)
        }
    }

    fn show_error<E>(mut self, error: &E)
        -> anyhow::Result<InteractionMachine<NoInteraction>>
    where E: std::error::Error
    {
        self.interface.show_error(error)?;
        Ok(self.into())
    }
}

impl From<InteractionMachine<DisabledInteraction>> for InteractionMachine<NoInteraction>
{
    fn from(current: InteractionMachine<DisabledInteraction>) -> Self
    {
        Self
        {
            interface: current.interface,
            cursor: current.cursor,
            interaction: NoInteraction
        }
    }
}

impl InteractionMachine<NoInteraction>
{
    fn refresh(mut self, event: &WindowEvent) -> anyhow::Result
    <
        Cases3
        <
            Self,
            InteractionMachine<DragInteraction>,
            InteractionMachine<ZoomInteraction>
        >
    >
    {
        match *event
        {
            WindowEvent::ScaleFactorChanged{scale_factor, ..} =>
                self.interface.set_scale_factor(scale_factor),
            WindowEvent::CursorMoved{position, ..} =>
                self.cursor = position,
            WindowEvent::MouseInput
            {
                state: ElementState::Pressed,
                button,
                ..
            } => match button
            {
                MouseButton::Left => return
                {
                    let this: InteractionMachine<_> = self.into();
                    this.interface.drag()?;
                    Ok(Cases3::B(this))
                },
                MouseButton::Right => return 
                    Ok(Cases3::C(self.try_into()?)),
                _ => {}
            }
            _ => {}
        }
        Ok(Cases3::A(self))
    }

    fn show_blank
    (
        &mut self,
        dimensions: PictureDimensions
    ) -> anyhow::Result<()>
    {
        self.interface.show_blank(dimensions)
    }

    fn show_picture(&mut self, still: StillPicture) -> anyhow::Result<()>
    {
        self.interface.show_picture(still)
            .or_else(|e| self.show_error(&e))
    }

    fn show_error<E>(&mut self, error: &E) -> anyhow::Result<()>
    where E: std::error::Error
    {
        self.interface.show_error(error)
    }
}


impl From<InteractionMachine<NoInteraction>> for InteractionMachine<DisabledInteraction>
{
    fn from(current: InteractionMachine<NoInteraction>) -> Self
    {
        Self
        {
            interface: current.interface,
            cursor: current.cursor,
            interaction: DisabledInteraction
        }
    }
}

impl From<InteractionMachine<NoInteraction>> for InteractionMachine<DragInteraction>
{
    fn from(current: InteractionMachine<NoInteraction>) -> Self
    {
        Self
        {
            interface: current.interface,
            cursor: current.cursor,
            interaction: DragInteraction
        }
    }
}

impl TryFrom<InteractionMachine<NoInteraction>> for InteractionMachine<ZoomInteraction>
{
    type Error = anyhow::Error;
    fn try_from
    (
        InteractionMachine{mut interface, cursor, ..}
            : InteractionMachine<NoInteraction>
    ) -> Result<Self, Self::Error>
    {
        interface.stamp.clear();
        interface.stamp.set_level(WindowLevel::AlwaysOnTop);
        spin(SPIN_TIME);
        interface.stamp.set_size(interface.get_window_size());
        interface.stamp.set_origin(interface.get_window_origin()?);
        interface.stamp.set_viewport(&interface.get_viewport().clone());
        interface.stamp.draw();
        spin(SPIN_TIME);
        let interaction = ZoomInteraction::new
        (
            &interface,
            cursor
        )?;
        let window_origin = interface.get_window_origin()?;
        let viewport = GLViewport
        {
            origin:
            [
                window_origin.x,
                interaction.screen_size_captured.height as i32 -
                (
                    window_origin.y +
                    interaction.window_size_captured.height as i32
                )
            ],
            size: interaction.window_size_captured.into()
        };
        interface.clear();
        spin(SPIN_TIME);
        interface.set_viewport(&viewport);
        interface.set_window_origin(PhysicalPosition{x: 0, y: 0});
        interface.set_window_size(interaction.screen_size_captured);
        interface.draw();
        spin(SPIN_TIME);
        interface.stamp.clear();
        interface.stamp.set_level(WindowLevel::AlwaysOnBottom);
        spin(SPIN_TIME);
        Ok(Self{interface, cursor, interaction})
    }
}

impl InteractionMachine<DragInteraction>
{
    fn refresh(mut self, event: &WindowEvent) -> anyhow::Result
    <
        Cases2
        <
            Self,
            InteractionMachine<NoInteraction>
        >
    >
    {
        match *event
        {
            WindowEvent::ScaleFactorChanged{scale_factor, ..} =>
                self.interface.set_scale_factor(scale_factor),
            WindowEvent::CursorMoved{position, ..} =>
                self.cursor = position,
            WindowEvent::MouseInput
            {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => return Ok(Cases2::B(self.into())),
            _ => {}
        }
        Ok(Cases2::A(self))
    }

    fn show_blank
    (
        &mut self,
        dimensions: PictureDimensions
    ) -> anyhow::Result<()>
    {
        self.interface.show_blank(dimensions)
    }

    fn show_picture(&mut self, still: StillPicture) -> anyhow::Result<()>
    {
        self.interface.show_picture(still)
            .or_else(|e| self.show_error(&e))
    }

    fn show_error<E>(&mut self, error: &E) -> anyhow::Result<()>
    where E: std::error::Error
    {
        self.interface.show_error(error)
    }
}

impl From<InteractionMachine<DragInteraction>> for InteractionMachine<NoInteraction>
{
    fn from(current: InteractionMachine<DragInteraction>) -> Self
    {
        Self
        {
            interface: current.interface,
            cursor: current.cursor,
            interaction: NoInteraction
        }
    }
}

impl InteractionMachine<ZoomInteraction>
{
    fn refresh(mut self, event: &WindowEvent) -> anyhow::Result
    <
        Cases2
        <
            Self,
            InteractionMachine<NoInteraction>
        >
    >
    {
        match *event
        {
            WindowEvent::ScaleFactorChanged{scale_factor, ..} =>
            {
                self.interface.set_scale_factor(scale_factor);
                return Ok(Cases2::B(self.into()))
            }
            WindowEvent::CursorMoved{position, ..} =>
            {
                self.cursor = position;
                let viewport = self.interaction.compute_viewport
                (
                    &self.interface,
                    &self.cursor
                )?;
                self.interface.set_viewport(&viewport);
                self.draw()
            }
            WindowEvent::MouseInput
            {
                state: ElementState::Released,
                button: MouseButton::Right,
                ..
            } => return Ok(Cases2::B(self.into())),
            _ => {}
        }
        Ok(Cases2::A(self))
    }

    fn show_blank
    (
        self,
        dimensions: PictureDimensions
    ) -> anyhow::Result<InteractionMachine<NoInteraction>>
    {
        let mut this: InteractionMachine<_> = self.into();
        this.interface.show_blank(dimensions).map(|_| this)
    }

    fn show_picture(mut self, still: StillPicture) -> anyhow::Result
    <
        Cases2
        <
            Self,
            InteractionMachine<NoInteraction>
        >
    >
    {
        match self.interface.show_picture(still)
        {
            Ok(()) => Ok(Cases2::A(self)),
            Err(error) => Ok(Cases2::B(self.show_error(&error)?))
        }
    }

    fn show_error<E>(self, error: &E)
        -> anyhow::Result<InteractionMachine<NoInteraction>>
    where E: std::error::Error
    {
        let mut this: InteractionMachine<_> = self.into();
        this.interface.show_error(error).map(|_| this)
    }
}

impl From<InteractionMachine<ZoomInteraction>> for InteractionMachine<NoInteraction>
{
    fn from
    (
        InteractionMachine{mut interface, cursor, interaction}
            : InteractionMachine<ZoomInteraction>
    ) -> Self
    {
        let mut viewport = interface.get_viewport().clone();
        let window_origin = PhysicalPosition
        {
            x: viewport.origin[0],
            y: interaction.screen_size_captured.height as i32 -
            (
                viewport.origin[1] +
                viewport.size[1] as i32
            )
        };
        let window_size: PhysicalSize<u32> = viewport.size.into();
        viewport.origin = [0; 2];
        interface.stamp.clear();
        interface.stamp.set_level(WindowLevel::AlwaysOnTop);
        spin(SPIN_TIME);
        interface.stamp.set_size(window_size);
        interface.stamp.set_origin(window_origin);
        interface.stamp.set_viewport(&viewport);
        interface.stamp.draw();
        spin(SPIN_TIME);
        interface.clear();
        spin(SPIN_TIME);
        interface.set_window_size(window_size);
        interface.set_window_origin(window_origin);
        interface.set_viewport(&viewport);
        interface.draw();
        spin(SPIN_TIME);
        interface.stamp.clear();
        interface.stamp.set_level(WindowLevel::AlwaysOnBottom);
        spin(SPIN_TIME);
        Self
        {
            interface,
            cursor,
            interaction: NoInteraction
        }
    }
}

// ------------------------------------------------------------

enum InterfaceEnum
{
    DisabledInteraction(InteractionMachine<DisabledInteraction>),
    NoInteraction(InteractionMachine<NoInteraction>),
    DragInteraction(InteractionMachine<DragInteraction>),
    ZoomInteraction(InteractionMachine<ZoomInteraction>)
}

impl fmt::Debug for InterfaceEnum
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result
    {
        match self
        {
            Self::DisabledInteraction(..) => write!
                (formatter, "Interface::DisabledInteraction"),
            Self::NoInteraction(..) => write!
                (formatter, "Interface::NoInteraction"),
            Self::DragInteraction(..) => write!
                (formatter, "Interface::DragInteraction"),
            Self::ZoomInteraction(..) => write!
                (formatter, "Interface::ZoomInteraction")
        }
    }
}

impl From<InteractionMachine<DisabledInteraction>> for InterfaceEnum
{
    fn from(machine: InteractionMachine<DisabledInteraction>) -> Self
    {
        Self::DisabledInteraction(machine)
    }
}

impl From<InteractionMachine<NoInteraction>> for InterfaceEnum
{
    fn from(machine: InteractionMachine<NoInteraction>) -> Self
    {
        Self::NoInteraction(machine)
    }
}

impl From<InteractionMachine<DragInteraction>> for InterfaceEnum
{
    fn from(machine: InteractionMachine<DragInteraction>) -> Self
    {
        Self::DragInteraction(machine)
    }
}

impl From<InteractionMachine<ZoomInteraction>> for InterfaceEnum
{
    fn from(machine: InteractionMachine<ZoomInteraction>) -> Self
    {
        Self::ZoomInteraction(machine)
    }
}

impl InterfaceEnum
{
    fn new(event_loop: &EventLoopWindowTarget<()>) -> anyhow::Result<Self>
    {
        InteractionMachine::new(event_loop)
            .map(Into::into)
    }

    fn window_id(&self) -> WindowId
    {
        match self
        {
            Self::DisabledInteraction(interaction)
                => interaction.window_id(),
            Self::NoInteraction(interaction)
                => interaction.window_id(),
            Self::DragInteraction(interaction)
                => interaction.window_id(),
            Self::ZoomInteraction(interaction)
                => interaction.window_id()
        }
    }

    fn refresh(self, event: &WindowEvent) -> anyhow::Result<Self>
    {
        match self
        {
            Self::DisabledInteraction(mut interaction) =>
            {
                interaction.refresh(event);
                Ok(interaction.into())
            }
            Self::NoInteraction(interaction) =>
                interaction.refresh(event).map
            (
                |cases| match cases
                {
                    Cases3::A(interaction) => interaction.into(),
                    Cases3::B(interaction) => interaction.into(),
                    Cases3::C(interaction) => interaction.into()
                }
            ),
            Self::DragInteraction(interaction) =>
                interaction.refresh(event).map
            (
                |cases| match cases
                {
                    Cases2::A(interaction) => interaction.into(),
                    Cases2::B(interaction) => interaction.into()
                }
            ),
            Self::ZoomInteraction(interaction) =>
                interaction.refresh(event).map
            (
                |cases| match cases
                {
                    Cases2::A(interaction) => interaction.into(),
                    Cases2::B(interaction) => interaction.into()
                }
            )
        }
    }

    fn disable_interaction(self) -> anyhow::Result<Self>
    {
        let this = match self
        {
            Self::DisabledInteraction(interaction) => interaction.into(),
            Self::NoInteraction(interaction) =>
                Self::DisabledInteraction(interaction.into()),
            Self::DragInteraction(..) => unimplemented!(),
            Self::ZoomInteraction(interaction) =>
            {
                let interaction: InteractionMachine<NoInteraction>
                    = interaction.try_into()?;
                Self::DisabledInteraction(interaction.into())
            }
        };
        Ok(this)
    }

    fn show_blank(mut self, dimensions: PictureDimensions) -> anyhow::Result<Self>
    {
        match self
        {
            Self::DisabledInteraction(interaction)
                => interaction.show_blank(dimensions)
                    .map(Into::into),
            Self::NoInteraction(ref mut interaction)
                => interaction.show_blank(dimensions)
                    .map(|_| self),
            Self::DragInteraction(ref mut interaction)
                => interaction.show_blank(dimensions)
                    .map(|_| self),
            Self::ZoomInteraction(interaction)
                => interaction.show_blank(dimensions)
                    .map(Into::into)
        }
    }

    fn show_picture(mut self, still: StillPicture) -> anyhow::Result<Self>
    {
        match self
        {
            Self::DisabledInteraction(interaction)
                => interaction.show_picture(still)
                    .map(Into::into),
            Self::NoInteraction(ref mut interaction)
                => interaction.show_picture(still)
                    .map(|_| self),
            Self::DragInteraction(ref mut interaction)
                => interaction.show_picture(still)
                    .map(|_| self),
            Self::ZoomInteraction(interaction)
                => interaction.show_picture(still).map
            (
                |cases| match cases
                {
                    Cases2::A(interaction) => interaction.into(),
                    Cases2::B(interaction) => interaction.into()
                }
            )
        }
    }

    fn show_error<E>(mut self, error: &E) -> anyhow::Result<Self>
    where E: std::error::Error
    {
        match self
        {
            Self::DisabledInteraction(interaction)
                => interaction.show_error(error)
                    .map(Into::into),
            Self::NoInteraction(ref mut interaction)
                => interaction.show_error(error)
                    .map(|_| self),
            Self::DragInteraction(ref mut interaction)
                => interaction.show_error(error)
                    .map(|_| self),
            Self::ZoomInteraction(interaction)
                => interaction.show_error(error)
                    .map(Into::into)
        }
    }

    fn is_error(&self) -> bool
    {
        match self
        {
            Self::DisabledInteraction(interaction)
                => interaction.is_error(),
            Self::NoInteraction(interaction)
                => interaction.is_error(),
            Self::DragInteraction(interaction)
                => interaction.is_error(),
            Self::ZoomInteraction(interaction)
                => interaction.is_error()
        }
    }

    fn draw(&mut self) -> ()
    {
        match self
        {
            Self::DisabledInteraction(interaction)
                => interaction.draw(),
            Self::NoInteraction(interaction)
                => interaction.draw(),
            Self::DragInteraction(interaction)
                => interaction.draw(),
            Self::ZoomInteraction(interaction)
                => interaction.draw()
        }   
    }
}

// ------------------------------------------------------------

pub struct Interface(InterfaceEnum);

impl Interface
{
    pub fn new(event_loop: &EventLoopWindowTarget<()>) -> anyhow::Result<Self>
    {
        InterfaceEnum::new(event_loop).map(Self)
    }

    pub fn window_id(&self) -> WindowId
    {
        self.0.window_id()
    }

    pub fn refresh(self, event: &WindowEvent) -> anyhow::Result<Self>
    {
        self.0.refresh(event).map(Self)
    }

    pub fn disable_interaction(self) -> anyhow::Result<Self>
    {
        self.0.disable_interaction().map(Self)
    }

    pub fn show_blank(self, dimensions: PictureDimensions) -> anyhow::Result<Self>
    {
        self.0.show_blank(dimensions).map(Self)
    }

    pub fn show_picture(self, still: StillPicture) -> anyhow::Result<Self>
    {
        self.0.show_picture(still).map(Self)
    }

    pub fn show_error<E>(self, error: &E) -> anyhow::Result<Self>
    where E: std::error::Error
    {
        self.0.show_error(error).map(Self)
    }

    pub fn is_error(&self) -> bool
    {
        self.0.is_error()
    }

    pub fn draw(&mut self) -> ()
    {
        self.0.draw()
    }
}
