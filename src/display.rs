
use std::path::PathBuf;
use super::{ogl::*, painters::*, picture, vector::*, quad::*};
use winit::{window::*, event_loop::*, dpi::*};
use raw_gl_context::*;
use raw_window_handle::*;

#[cfg(target_os = "windows")]
use windows::
{
    core::PSTR, 
    Win32::
    {
        self,
        UI::ColorSystem::GetICMProfileA,
        Foundation::
        {
            GetLastError,
            WIN32_ERROR
        }
    }
};

// ----------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub enum QuerryMonitorICCError
{
    UnsupportedOSError,
    Win32Error(WIN32_ERROR),
    UnknownWindowsError,
    FromUtf8Error(std::string::FromUtf8Error)
}

// ----------------------------------------------------------------------------------------------------

impl From<Vector2> for [u32; 2]
{
    fn from(vector: Vector2) -> Self
    {
        [
            vector.0[0].round() as _, 
            vector.0[1].round() as _
        ]
    }
}

impl From<Vector2> for [i32; 2]
{
    fn from(vector: Vector2) -> Self
    {
        [
            vector.0[0].round() as _, 
            vector.0[1].round() as _
        ]
    }
}

// ----------------------------------------------------------------------------------------------------

struct XDisplay
{
    size: LogicalSize<f32>,
    canvas: Canvas,
    filler: Filler,
    window: Quad,
    x: Quad
}

impl XDisplay
{
    fn new(pointers: &FunctionPointers) -> Self
    {
        let window = Quad::new([0.0, 0.0], [500.0, 500.0]);
        let mut x = Quad::new([0.0, 0.0], [33.0, 33.0]);
        x.center(&window);
        Self
        {
            size: LogicalSize::<f32>::from(window.size().0),
            filler: Filler::new(pointers),
            canvas: Canvas::new
            (
                pointers,
                &"
                #version 330 core
                in vec2 st;
                out vec4 color;
                void main()
                {
                    float w = 0.036;
                    float s = 0.015;
                    float r;
                    for(int d = 0; d < 2; d++)
                    {
                        float x = bool(d) ? st.x : 1.0 - st.x;
                        r += smoothstep(1.0 - w, 1.0 - w + s, x + st.y)
                            * smoothstep(1.0 + w, 1.0 + w - s, x + st.y);
                    }
                    color = vec4(vec3(1.0 - r), 0.8);
                }
                "
            ),
            window,
            x
        }
    }

    fn draw(&mut self, scale: f64) -> ()
    {
        let (mut window, mut x) = (self.window, self.x);
        window.scale(Vector([scale, scale]));
        x.scale(Vector([scale, scale]));
        self.filler.fill
        (
            [1.0; 4], 
            window.min().into(), 
            window.size().into()
        );
        self.canvas.draw
        (
            x.min().into(),
            x.size().into()
        )
    }
}

// ----------------------------------------------------------------------------------------------------

struct Loader(Filler);

impl Loader
{
    fn new(pointers: &FunctionPointers) -> Self
    {
        Self(Filler::new(pointers))
    }

    fn draw(&mut self, size: PhysicalSize<u32>) -> ()
    {
        self.0.fill
        (
            [0.0, 0.0, 0.0, 1.0], 
            [0, 0], 
            [size.width, size.height]
        )
    }
}

// ----------------------------------------------------------------------------------------------------

struct PictureDisplay(Blitter);

impl PictureDisplay
{
    fn new(pointers: &FunctionPointers) -> Self
    {
        Self(Blitter::new(pointers))
    }
    
    fn setup(&mut self, still: &picture::StillPicture) -> ()
    {
        match &still.pixel_data
        {
            picture::PixelData::EightBit(data)
                => self.0.upload_texture
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
                => self.0.upload_texture
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
                => self.0.upload_texture
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
    
    fn draw(&mut self, size: PhysicalSize<u32>) -> ()
    {
        self.0.blit([size.width, size.height])
    }
}

// ----------------------------------------------------------------------------------------------------

enum RenderMode
{
    Uninitialized,
    Loader,
    Picture,
    Error
}

// ----------------------------------------------------------------------------------------------------

struct Renderer
{
    loder: Loader,
    picture: PictureDisplay,
    error: XDisplay,
    mode: RenderMode
}

impl Renderer
{
    fn new(pointers: &FunctionPointers) -> Self
    {
        Self
        {
            loder: Loader::new(pointers),
            picture: PictureDisplay::new(pointers),
            error: XDisplay::new(pointers),
            mode: RenderMode::Uninitialized
        }
    }
    
    fn use_loader(&mut self) -> ()
    {
        self.mode = RenderMode::Loader
    }

    fn prepare_error(&mut self) -> LogicalSize<f32>
    {
        self.mode = RenderMode::Error;
        self.error.size
    }

    fn prepare_picture(&mut self, still: &picture::StillPicture) -> ()
    {
        self.mode = RenderMode::Picture;
        self.picture.setup(still)
    }

    fn draw
    (
        &mut self, 
        size: PhysicalSize<u32>, 
        scale_factor: f64
    ) -> ()
    {
        use RenderMode::*;
        match &self.mode
        {
            Loader => self.loder.draw(size),
            Picture => self.picture.draw(size),
            Error => self.error.draw(scale_factor),
            _ => {}
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
    fn new() -> (Self, EventLoop<()>)
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
            .unwrap();
        let context = unsafe 
        {
            let context = GlContext::create
            (
                &window, 
                Default::default()
            ).unwrap();
            context.make_current();
            context
        };
        let pointers = FunctionPointers
            ::load(|s| context.get_proc_address(s));
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
        let mut this = Self{window, context, pointers};
        let size = this.screen_size();
        this.set_size(size);
        this.set_position(PhysicalPosition::new(0, 0));
        (this, event_loop)
    }
    
    fn querry_monitor_icc(&self) -> std::result::Result
    <
        PathBuf,
        QuerryMonitorICCError
    >
    {
        if let RawWindowHandle::Win32(handle) = self.window.raw_window_handle()
        {
            if cfg!(target_os = "windows")
            {
                let hwnd = Win32::Foundation::HWND(handle.hwnd as _);
                let mut buffer_size: u32 = 0;
                return unsafe
                {
                    let hdc = Win32::Graphics::Gdi::GetDC(hwnd);
                    match GetICMProfileA
                    (
                        hdc,
                        &mut buffer_size as *mut u32,
                        PSTR::null()
                    ).as_bool()
                    {
                        true => Err(QuerryMonitorICCError::UnknownWindowsError),
                        false => match GetLastError()
                        {
                            WIN32_ERROR(122) => // ERROR_INSUFFICIENT_BUFFER
                            {
                                let mut filename: Vec<u8> = vec![0; buffer_size as _];
                                let pszfilename = PSTR(filename.as_mut_ptr());
                                match GetICMProfileA
                                (
                                    hdc,
                                    &mut buffer_size as *mut u32,
                                    pszfilename
                                ).as_bool()
                                {
                                    true => match pszfilename.to_string()
                                    {
                                        Ok(path) => Ok(PathBuf::from(path)),
                                        Err(error) => Err(QuerryMonitorICCError::FromUtf8Error(error))
                                    }
                                    false => Err(QuerryMonitorICCError::Win32Error(GetLastError()))
                                }
                            }
                            error @ _ => Err(QuerryMonitorICCError::Win32Error(error))
                        }
                    }
                }
            }
        };
        Err(QuerryMonitorICCError::UnsupportedOSError)
    }

    fn scale_factor(&self) -> f64
    {
         self.window.scale_factor()
    }
    
    fn size(&self) -> PhysicalSize<u32>
    {
        self.window.outer_size()
    }
    
    fn position(&self) -> PhysicalPosition<i32>
    {
        self.window
            .outer_position()
            .unwrap()
    }
    
    fn screen_size(&self) -> PhysicalSize<u32>
    {
         self.window
            .current_monitor()
            .unwrap()
            .size()
    }
    
    fn visible(&self, visible: bool) -> ()
    {
        self.window.set_visible(visible)
    }

    fn drag(&self) -> ()
    {
        self.window.drag_window().unwrap()
    }
    
    fn get_center(&self) -> PhysicalPosition<i32>
    {
        let mut position = self.position();
        let size = self.size();
        position.x += (size.width as f32 * 0.5).round() as i32;
        position.y += (size.height as f32 * 0.5).round() as i32;
        position
    }

    fn resize_overflow_to_screen(&mut self, scale: f32) -> ()
    {
        let screen = self.screen_size();
        let screen = (screen.width as f32, screen.height as f32);
        let window = self.size();
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
        self.set_size(PhysicalSize::<f32>::from(fitted))
    }
    
    fn set_position<P: Into<Position>>(&self, position: P) -> ()
    {
        self.window.set_outer_position(position)
    }

    fn set_size<S: Into<Size>>(&mut self, size: S) -> ()
    {
        self.window.set_inner_size(size) // implicit `RedrawRequested`
    }
}

// ----------------------------------------------------------------------------------------------------

pub struct Display
{
    window: GLWindow,
    renderer: Renderer,
    size: Size,
    icc: lcms2::Profile
}

impl Display
{
    pub fn new() -> (Self, EventLoop<()>)
    {
        let (window, event_loop) = GLWindow::new();
        let renderer = Renderer::new(&window.pointers);
        let icc = match window.querry_monitor_icc()
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
        let this = Self
        {
            window, 
            renderer,
            size: PhysicalSize::new(0, 0).into(),
            icc
        };
        (this, event_loop)
    }
    
    pub fn get_icc(&self) -> &lcms2::Profile
    {
        &self.icc
    }

    pub fn visible(&self, visible: bool) -> ()
    {
        self.window.visible(visible)
    }
    
    pub fn drag(&self) -> ()
    {
        self.window.drag()
    }

    fn request_draw<S: Into<Size>>(&mut self, size: S) -> ()
    {
        let previous_center = self.window.get_center();
        self.window.set_size(size);
        self.window.resize_overflow_to_screen(0.8);
        self.size = self.window.size().into();
        let mut position = self.window.position();
        let new_center = self.window.get_center();
        position.x -= new_center.x - previous_center.x;
        position.y -= new_center.y - previous_center.y;
        self.window.set_position(position)
    }

    pub fn show_loader(&mut self, size: PhysicalSize<u32>) -> ()
    {
        self.renderer.use_loader();
        self.request_draw(size)
    }

    pub fn show_picture(&mut self, still: &picture::StillPicture) -> ()
    {
        self.renderer.prepare_picture(still);
        let size = PhysicalSize::<u32>::from(still.resolution);
        self.request_draw(size)
    }
    
    pub fn show_x<E>(&mut self, error: &E) -> ()
    where
        E: std::error::Error
    {
        eprintln!("{:?}", error);
        let size = self.renderer.prepare_error();
        self.request_draw(size)
    }

    pub fn draw(&mut self) -> ()
    {
        self.window.set_size(self.size);
        let scale_factor = self.window.scale_factor();
        let size = self.size.to_physical::<u32>(scale_factor);
        unsafe{self.window.pointers.Clear(COLOR_BUFFER_BIT)}
        self.renderer.draw(size, scale_factor);
        self.window.context.swap_buffers()
    }
}
