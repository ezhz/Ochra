
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

// ------------------------------------------------------------

const FONT: &[u8] = include_bytes!("../assets/font.ttf");

// ------------------------------------------------------------

struct ErrorPainter
{
    filler: Filler,
    viewport: GLViewport,
    typewriter: Typewriter
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
            viewport: GLViewport
            {
                origin: [0; 2],
                size: 
                [
                    Self::SIZE.width.round() as _,
                    Self::SIZE.height.round() as _
                ]
            }
        }
    }

    fn set_message(&mut self, message: &str) -> ()
    {
        self.typewriter.layout_text(message, 60)
    }

    fn get_size(&self) -> PhysicalSize<u32>
    {
        self.viewport.size.into()
    }

    fn set_scale_factor(&mut self, scale_factor: f32) -> ()
    {
        self.typewriter.change_font_size
        (
            (16.0 * scale_factor).round() as _
        );
        self.viewport.size =
        [
            (Self::SIZE.width * scale_factor).round() as _,
            (Self::SIZE.height * scale_factor).round() as _
        ]
    }

    fn draw(&mut self) -> ()
    {
        self.filler.fill
        (
            [1.0; 4], 
            &self.viewport
        );
        self.typewriter.draw
        (
            [
                (
                    self.viewport.size[0] as f32 * 0.5 -
                    self.typewriter.dimensions()[0] as f32 * 0.5
                ).round() as _,
                (
                    self.viewport.size[1] as f32 * 0.5 -
                    self.typewriter.dimensions()[1] as f32 * 0.5
                ).round() as _
            ]
        )
    }
}

// ------------------------------------------------------------

struct BlankPainter(Filler);

impl BlankPainter
{
    fn new(pointers: &FunctionPointers) -> Self
    {
        Self(Filler::new(pointers))
    }

    fn draw(&mut self, viewport: &GLViewport) -> ()
    {
        self.0.fill
        (
            [0.0, 0.0, 0.0, 1.0], 
            viewport
        )
    }
}

// ------------------------------------------------------------

struct PicturePainter(Blitter);

impl PicturePainter
{
    fn new(pointers: &FunctionPointers) -> Self
    {
        Self(Blitter::new(pointers))
    }
    
    fn set_picture(&mut self, still: &picture::StillPicture) -> ()
    {
        match &still.pixel_data
        {
            picture::PixelData::EightBit(data) => self.0.upload_texture
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
            picture::PixelData::SixteenBit(data) => self.0.upload_texture
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
            )
        }
    }

    fn draw(&mut self, viewport: &GLViewport) -> ()
    {
        self.0.blit(viewport)
    }
}

// ------------------------------------------------------------

enum RenderMode
{
    Blank,
    Picture,
    Error
}

// ------------------------------------------------------------

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
    
    fn set_scale_factor(&mut self, scale_factor: f32) -> ()
    {
        self.error.set_scale_factor(scale_factor)
    }

    fn use_blank_mode(&mut self) -> ()
    {
        self.mode = RenderMode::Blank
    }

    fn use_picture_mode
    (
        &mut self,
        still: &picture::StillPicture
    ) -> ()
    {
        self.mode = RenderMode::Picture;
        self.picture.set_picture(still)
    }

    fn use_error_mode<E>(&mut self, error: &E) -> ()
    where E: std::error::Error
    {
        self.mode = RenderMode::Error;
        self.error.set_message(&error.to_string())
    }

    fn get_error_box_size(&self) -> PhysicalSize<u32>
    {
        self.error.get_size()
    }

    fn draw(&mut self, viewport: &GLViewport) -> ()
    {
        match &self.mode
        {
            RenderMode::Blank => self.blank.draw(viewport),
            RenderMode::Picture => self.picture.draw(viewport),
            RenderMode::Error => self.error.draw()
        }
    }
}

// ------------------------------------------------------------

struct GLWindow
{
    window: Window,
    context: GlContext,
    pointers: FunctionPointers
}

impl GLWindow
{
    fn new
    (
        event_loop: &EventLoopWindowTarget<()>
    ) -> anyhow::Result<Self>
    {
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
        Ok(Self{window, context, pointers})
    }

    #[cfg(target_os = "windows")]
    fn query_monitor_icc(&self) -> anyhow::Result<PathBuf>
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

    #[cfg(not(target_os = "windows"))]
    fn query_monitor_icc(&self) -> anyhow::Result<PathBuf>
    {
        bail!("Could not query monitor ICC. Unsupported OS.")
    }

    fn make_context_current(&self) -> ()
    {
        unsafe{self.context.make_current()}
    }

    fn set_visible(&self, visible: bool) -> ()
    {
        self.window.set_visible(visible)
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

    fn get_origin(&self) -> Result
    <
        PhysicalPosition<i32>,
        winit::error::NotSupportedError
    >
    {
        self.window.outer_position()
    }
    
    fn set_origin<O: Into<Position>>(&self, origin: O) -> ()
    {
        self.window.set_outer_position(origin)
    }

    fn get_screen_size(&self) -> anyhow::Result<PhysicalSize<u32>>
    {
         self.window.current_monitor()
            .context("Could not detect current monitor")
            .map(|m| m.size())
    }

    fn get_center(&self) -> Result
    <
        PhysicalPosition<i32>,
        winit::error::NotSupportedError
    >
    {
        let mut position = self.get_origin()?;
        let size = self.get_size();
        position.x += (size.width as f32 * 0.5).round() as i32;
        position.y += (size.height as f32 * 0.5).round() as i32;
        Ok(position)
    }

    #[must_use]
    fn drag(&self) -> anyhow::Result<()>
    {
        self.window.drag_window()
            .context("Could not drag window")
    }
}

// ------------------------------------------------------------

pub struct RenderWindow
{
    window: GLWindow,
    viewport: GLViewport,
    renderer: Renderer,
    icc: lcms2::Profile
}

impl RenderWindow
{
    pub fn new(event_loop: &EventLoopWindowTarget<()>) -> anyhow::Result<Self>
    {
        let window = GLWindow::new(&event_loop)?;
        let renderer = Renderer::new(&window.pointers);
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
        (
            Self
            {
                window,
                viewport: Default::default(),
                renderer,
                icc
            }
        )
    }

    pub fn get_monitor_icc(&self) -> &lcms2::Profile
    {
        &self.icc
    }
    
    pub fn set_visible(&self, visible: bool) -> ()
    {
        self.window.set_visible(visible)
    }

    pub fn get_scale_factor(&self) -> f64
    {
         self.window.get_scale_factor()
    }

    pub fn get_size(&self) -> PhysicalSize<u32>
    {
        self.window.get_size()
    }

    // implicit winit::event::Event::RedrawRequested
    pub fn set_size<S: Into<Size>>(&mut self, size: S) -> ()
    {
        self.window.set_size(size)
    }

    pub fn get_origin(&self) -> Result
    <
        PhysicalPosition<i32>,
        winit::error::NotSupportedError
    >
    {
        self.window.get_origin()
    }

    pub fn set_origin<O: Into<Position>>(&self, origin: O) -> ()
    {
        self.window.set_origin(origin)
    }

    pub fn get_screen_size(&self) -> anyhow::Result<PhysicalSize<u32>>
    {
        self.window.get_screen_size()
    }

    pub fn get_center(&self) -> Result
    <
        PhysicalPosition<i32>,
        winit::error::NotSupportedError
    >
    {
        self.window.get_center()
    }

    pub fn get_viewport(&self) -> &GLViewport
    {
        &self.viewport
    }

    pub fn set_viewport(&mut self, viewport: GLViewport) -> ()
    {
        self.viewport = viewport
    }

    pub fn set_scale_factor(&mut self, scale_factor: f64) -> ()
    {
        self.renderer.set_scale_factor(scale_factor as _)
    }

    pub fn use_blank_mode(&mut self) -> ()
    {
        self.renderer.use_blank_mode()
    }

    pub fn use_picture_mode(&mut self, still: &picture::StillPicture) -> ()
    {
        self.window.make_context_current();
        self.renderer.use_picture_mode(still)
    }
    
    pub fn use_error_mode<E>(&mut self, error: &E) -> ()
    where E: std::error::Error
    {
        eprintln!("{:?}", error);
        self.renderer.use_error_mode(error)
    }

    pub fn is_error(&self) -> bool
    {
        if let RenderMode::Error = self.renderer.mode
        {
            return true
        }
        false
    }

    pub fn get_error_box_size(&self) -> PhysicalSize<u32>
    {
        self.renderer.get_error_box_size()
    }

    pub fn drag(&self) -> anyhow::Result<()>
    {
        self.window.drag()
    }

    pub fn clear(&self) -> ()
    {
        self.window.make_context_current();
        unsafe{self.window.pointers.Clear(COLOR_BUFFER_BIT)}
        self.window.context.swap_buffers()
    }

    pub fn draw(&mut self) -> ()
    {
        self.window.make_context_current();
        unsafe{self.window.pointers.Clear(COLOR_BUFFER_BIT)}
        self.renderer.draw(&self.viewport);
        self.window.context.swap_buffers()
    }
}
