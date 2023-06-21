
use 
{
    std::
    {
        path::PathBuf,
        result::Result
    },
    super::
    {
        ogl::*,
        painters::*,
        picture
    },
    winit::{window::*, event_loop::*, dpi::*},
    raw_gl_context::*,
    raw_window_handle::*,
    anyhow::{Context, bail}
};

// ----------------------------------------------------------------------------------------------------

const FONT: &[u8] = include_bytes!("../assets/font.ttf");

// ----------------------------------------------------------------------------------------------------

struct ErrorPainter
{
    filler: Filler,
    typewriter: Typewriter,
    size: [u32; 2]
}

impl ErrorPainter
{
    const SIZE: LogicalSize<f32> = LogicalSize
    {
        width: 500.0,
        height: 500.0
    };

    fn new(pointers: &FunctionPointers) -> Self
    {
        Self
        {
            filler: Filler::new(pointers),
            typewriter: Typewriter::new
            (
                pointers,
                FONT,
                16
            ),
            size:
            [
                Self::SIZE.width.round() as _,
                Self::SIZE.height.round() as _
            ]
        }
    }

    fn set_message(&mut self, message: &str) -> ()
    {
        self.typewriter.layout_text(message, 60)
    }

    fn get_size(&self) -> PhysicalSize<u32>
    {
        self.size.into()
    }

    fn set_scale_factor(&mut self, scale_factor: f32) -> ()
    {
        self.typewriter.change_font_size
        (
            (16.0 * scale_factor).round() as _
        );
        self.size =
        [
            (Self::SIZE.width * scale_factor).round() as _,
            (Self::SIZE.height * scale_factor).round() as _
        ]
    }

    fn draw(&mut self) -> ()
    {
        self.filler.fill([1.0; 4], [0; 2], self.size);
        self.typewriter.draw
        (
            [
                (
                    self.size[0] as f32 * 0.5 -
                    self.typewriter.dimensions()[0] as f32 * 0.5
                ).round() as _,
                (
                    self.size[1] as f32 * 0.5 -
                    self.typewriter.dimensions()[1] as f32 * 0.5
                ).round() as _
            ]
        )
    }
}

// ----------------------------------------------------------------------------------------------------

struct BlankPainter
{
    filler: Filler,
    size: [u32; 2]
}

impl BlankPainter
{
    fn new(pointers: &FunctionPointers) -> Self
    {
        Self
        {
            filler: Filler::new(pointers),
            size: Default::default()
        }
    }

    fn get_size(&self) -> PhysicalSize<u32>
    {
        self.size.into()
    }

    fn set_size(&mut self, size: PhysicalSize<u32>) -> ()
    {
        self.size = [size.width, size.height]
    }

    fn draw(&mut self) -> ()
    {
        self.filler.fill
        (
            [0.0, 0.0, 0.0, 1.0], 
            [0, 0], 
            self.size
        )
    }
}

// ----------------------------------------------------------------------------------------------------

struct PicturePainter
{
    blitter: Blitter,
    size: [u32; 2]
}

impl PicturePainter
{
    fn new(pointers: &FunctionPointers) -> Self
    {
        Self
        {
            blitter: Blitter::new(pointers),
            size: Default::default()
        }
    }
    
    fn get_size(&self) -> PhysicalSize<u32>
    {
        self.size.into()
    }

    fn set_size(&mut self, size: PhysicalSize<u32>) -> ()
    {
        self.size = [size.width, size.height]
    }

    fn set_picture(&mut self, still: &picture::StillPicture) -> ()
    {
        match &still.pixel_data
        {
            picture::PixelData::EightBit(data)
                => self.blitter.upload_texture
            (
                Image::<u8>
                {
                    data: Some(data),
                    resolution: still.resolution,
                    channel_count: still.channel_count
                },
                still.channel_interpretation
                    .swizzle_for_rgba(),
                still.gamma
            ),
            picture::PixelData::SixteenBit(data)
                => self.blitter.upload_texture
            (
                Image::<u16>
                {
                    data: Some(data), 
                    resolution: still.resolution, 
                    channel_count: still.channel_count
                },
                still.channel_interpretation
                    .swizzle_for_rgba(),
                still.gamma
            ),
            picture::PixelData::ThirtyTwoBit(data)
                => self.blitter.upload_texture
            (
                Image::<f32>
                {
                    data: Some(data), 
                    resolution: still.resolution, 
                    channel_count: still.channel_count
                },
                still.channel_interpretation
                    .swizzle_for_rgba(),
                still.gamma
            )
        }
    }

    fn draw(&mut self) -> ()
    {
        self.blitter.blit(self.size)
    }
}

// ----------------------------------------------------------------------------------------------------

enum RenderMode
{
    Blank,
    Picture,
    Error
}

// ----------------------------------------------------------------------------------------------------

struct Renderer
{
    blank: BlankPainter,
    picture: PicturePainter,
    error: ErrorPainter,
    mode: RenderMode
}

impl Renderer
{
    fn new(pointers: &FunctionPointers) -> Self
    {
        Self
        {
            blank: BlankPainter::new(pointers),
            picture: PicturePainter::new(pointers),
            error: ErrorPainter::new(pointers),
            mode: RenderMode::Blank
        }
    }
    
    fn use_blank(&mut self, size: PhysicalSize<u32>) -> ()
    {
        self.mode = RenderMode::Blank;
        self.blank.set_size(size)
    }

    fn use_picture
    (
        &mut self,
        still: &picture::StillPicture,
        size: PhysicalSize<u32>
    ) -> ()
    {
        self.mode = RenderMode::Picture;
        self.picture.set_picture(still);
        self.picture.set_size(size)
    }

    fn use_error<E>(&mut self, error: &E) -> PhysicalSize<u32>
    where E: std::error::Error
    {
        self.mode = RenderMode::Error;
        self.error.set_message(&error.to_string());
        self.error.get_size()
    }

    fn get_size(&self) -> PhysicalSize<u32>
    {
        match &self.mode
        {
            RenderMode::Blank => self.blank.get_size(),
            RenderMode::Picture => self.picture.get_size(),
            RenderMode::Error => self.error.get_size()
        }
    }

    fn set_scale_factor(&mut self, scale_factor: f64) -> ()
    {
        self.error.set_scale_factor(scale_factor as _)
    }

    fn draw(&mut self) -> ()
    {
        match &self.mode
        {
            RenderMode::Blank => self.blank.draw(),
            RenderMode::Picture => self.picture.draw(),
            RenderMode::Error => self.error.draw()
        }
    }
}

// ----------------------------------------------------------------------------------------------------

struct GLWindow
{
    window: Window,
    context: GlContext,
    pointers: FunctionPointers
}

impl GLWindow
{
    fn new() -> anyhow::Result<(Self, EventLoop<()>)>
    {
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_visible(false)
            .with_title("")
            .with_maximized(false)
            .with_transparent(true)
            .with_window_icon(None)
            .with_decorations(false)
            .with_resizable(false)
            .build(&event_loop)
            .context("Could not create openGL window")?;
        let context = unsafe 
        {
            let context = GlContext::create
            (
                &window, 
                Default::default()
            )?;
            context.make_current();
            context
        };
        let pointers = FunctionPointers::load
        (
            |s| context.get_proc_address(s)
        );
        unsafe
        {
            pointers.ClearColor(0.0, 0.0, 0.0, 0.0);
            pointers.Enable(BLEND);
            pointers.BlendFuncSeparate
            (
                SRC_ALPHA, 
                ONE_MINUS_SRC_ALPHA, 
                ONE, 
                ONE
            );
            pointers.PixelStorei(UNPACK_ALIGNMENT, 1);
            pointers.PixelStorei(PACK_ALIGNMENT, 1);
        }
        Ok
        ((
            Self{window, context, pointers},
            event_loop
        ))
    }
    
    fn query_monitor_icc(&self) -> anyhow::Result<PathBuf>
    {
        if cfg!(target_os = "windows")
        {
            if let RawWindowHandle::Win32(wh) = self.window.raw_window_handle()
            {
                let wh = windows::Win32::Foundation::HWND(wh.hwnd as _);
                let mut buffer_len: u32 = 0;
                return unsafe
                {
                    let hdc = windows::Win32::Graphics::Gdi::GetDC(wh);
                    match windows::Win32::UI::ColorSystem::GetICMProfileA
                    (
                        hdc,
                        &mut buffer_len as *mut u32,
                        windows::core::PSTR::null()
                    ).as_bool()
                    {
                        true => bail!("Could not query monitor ICC. Unknown error."),
                        false => match windows::Win32::Foundation::GetLastError()
                        {
                            windows::Win32::Foundation::WIN32_ERROR(122) => // ERROR_INSUFFICIENT_BUFFER
                            {
                                let mut filename: Vec<u8> = vec![0; buffer_len as _];
                                let pszfilename = windows::core::PSTR(filename.as_mut_ptr());
                                match windows::Win32::UI::ColorSystem::GetICMProfileA
                                (
                                    hdc,
                                    &mut buffer_len as *mut _,
                                    pszfilename
                                ).as_bool()
                                {
                                    true => pszfilename.to_string()
                                        .context("Could not query monitor ICC")
                                        .map(PathBuf::from),
                                    false => bail!
                                    (
                                        "Could not query monitor ICC. {:?}",
                                        windows::Win32::Foundation::GetLastError()
                                    )
                                }
                            }
                            error @ _ => bail!("Could not query monitor ICC. {error:?}")
                        }
                    }
                }
            };
            bail!("Could not query monitor ICC. Could not get window handle.")
        }
        bail!("Could not query monitor ICC. Unsupported OS.")
    }

    fn get_scale_factor(&self) -> f64
    {
         self.window.scale_factor()
    }
    
    fn get_size(&self) -> PhysicalSize<u32>
    {
        self.window.outer_size()
    }

    // implicit winit::event::Event::RedrawRequested
    fn set_size<S: Into<Size>>(&mut self, size: S) -> ()
    {
        self.window.set_inner_size(size)
    }

    fn get_position(&self) -> Result
    <
        PhysicalPosition<i32>,
        winit::error::NotSupportedError
    >
    {
        self.window.outer_position()
    }
    
    fn set_position<P: Into<Position>>(&self, position: P) -> ()
    {
        self.window.set_outer_position(position)
    }

    fn get_screen_size(&self) -> anyhow::Result<PhysicalSize<u32>>
    {
         self.window.current_monitor()
            .context("Could not detect current monitor")
            .map(|m| m.size())
    }
    
    fn set_visible(&self, visible: bool) -> ()
    {
        self.window.set_visible(visible)
    }

    fn get_center(&self) -> Result
    <
        PhysicalPosition<i32>,
        winit::error::NotSupportedError
    >
    {
        let mut position = self.get_position()?;
        let size = self.get_size();
        position.x += (size.width as f32 * 0.5).round() as i32;
        position.y += (size.height as f32 * 0.5).round() as i32;
        Ok(position)
    }

    #[must_use]
    fn drag(&self) -> anyhow::Result<()>
    {
        self.window.drag_window().context("Could not drag window")
    }

    fn fit_overflow_to_screen(&mut self, scale: f32) -> anyhow::Result<()>
    {
        let screen = self.get_screen_size()?;
        let screen = (screen.width as f32, screen.height as f32);
        let window = self.get_size();
        let window = (window.width as f32, window.height as f32);
        let mut fitted = window;
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
        Ok(self.set_size(PhysicalSize::<f32>::from(fitted)))
    }
}

// ----------------------------------------------------------------------------------------------------

pub struct Display
{
    window: GLWindow,
    renderer: Renderer,
    icc: lcms2::Profile
}

impl Display
{
    pub fn new() -> anyhow::Result<(Self, EventLoop<()>)>
    {
        let (window, event_loop) = GLWindow::new()?;
        let mut renderer = Renderer::new(&window.pointers);
        renderer.set_scale_factor(window.get_scale_factor());
        let icc = match window.query_monitor_icc()
        {
            Ok(path) => match lcms2::Profile::new_file(path)
            {
                Ok(profile) => profile,
                Err(error) =>
                {
                    eprintln!("{error:?}");
                    lcms2::Profile::new_srgb()
                }
            }
            Err(error) =>
            {
                eprintln!("{error:?}");
                lcms2::Profile::new_srgb()
            }
        };
        Ok
        ((
            Self
            {
                window, 
                renderer,
                icc
            },
            event_loop
        ))
    }
    
    pub fn get_icc(&self) -> &lcms2::Profile
    {
        &self.icc
    }

    pub fn set_visible(&self, visible: bool) -> ()
    {
        self.window.set_visible(visible)
    }
    
    fn get_size(&self) -> PhysicalSize<u32>
    {
        self.window.get_size()
    }

    fn set_size(&mut self, size: PhysicalSize<u32>, fit: bool) -> anyhow::Result<()>
    {
        let previous_center = self.window.get_center()?;
        self.window.set_size(size);
        if fit
        {
            self.window.fit_overflow_to_screen(0.8)?
        }
        let mut position = self.window.get_position()?;
        let new_center = self.window.get_center()?;
        position.x -= new_center.x - previous_center.x;
        position.y -= new_center.y - previous_center.y;
        Ok(self.window.set_position(position))
    }

    pub fn set_scale_factor(&mut self, scale_factor: f64) -> ()
    {
        self.renderer.set_scale_factor(scale_factor)
    }

    pub fn show_blank(&mut self, size: PhysicalSize<u32>) -> anyhow::Result<()>
    {
        self.set_size(size, true)?;
        self.renderer.use_blank(self.get_size());
        Ok(())
    }

    pub fn show_picture(&mut self, still: &picture::StillPicture) -> anyhow::Result<()>
    {
        self.set_size(still.resolution.into(), true)?;
        self.renderer.use_picture(still, self.get_size());
        Ok(())
    }
    
    pub fn show_error<E>(&mut self, error: &E) -> anyhow::Result<()>
    where
        E: std::error::Error
    {
        eprintln!("{:?}", error);
        let size = self.renderer.use_error(error);
        self.set_size(size, false)
    }

    pub fn drag(&self) -> anyhow::Result<()>
    {
        self.window.drag()
    }

    pub fn draw(&mut self) -> anyhow::Result<()>
    {
        self.set_size(self.renderer.get_size(), false)?;
        unsafe{self.window.pointers.Clear(COLOR_BUFFER_BIT)}
        self.renderer.draw();
        self.window.context.swap_buffers();
        Ok(())
    }
}
