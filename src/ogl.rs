
use std::{ffi::{CString, c_void}, ops::Deref, rc::Rc, fmt};

// ----------------------------------------------------------------------------------------------------

mod bindings{include!{concat!{env!{"OUT_DIR"}, "/gl_bindings.rs"}}}
pub use bindings::{*, types::*};

// ----------------------------------------------------------------------------------------------------

#[derive(Clone)]
pub struct FunctionPointers(Rc<Gl>);

impl FunctionPointers
{    
    pub fn load<F>(pointer_loader: F) -> Self
    where
        F: FnMut(&'static str) -> *const c_void
    {
        let pointers = Gl::load_with(pointer_loader);
        unsafe{pointers.Disable(FRAMEBUFFER_SRGB)}
        Self(Rc::new(pointers))
    }
}

impl Deref for FunctionPointers
{
    type Target = Gl;
    fn deref(&self) -> &Self::Target
    { 
        &self.0
    }
}

// ----------------------------------------------------------------------------------------------------

pub struct Buffer
{ 
    pointers: FunctionPointers,
    handle: GLuint
}

impl Buffer
{
    pub fn new(pointers: &FunctionPointers) -> Self
    {
        let mut handle = 0;
        unsafe{pointers.GenBuffers(1, &mut handle)}
        Self{pointers: pointers.clone(), handle}
    }
}

impl Drop for Buffer
{
    fn drop(&mut self) -> ()
    {
        unsafe{self.pointers.DeleteBuffers(1, &self.handle)}
    }
}

impl Deref for Buffer
{
    type Target = GLuint;
    fn deref(&self) -> &Self::Target
    {
        &self.handle
    }
}

// ----------------------------------------------------------------------

pub struct VertexArrayObject
{ 
    pointers: FunctionPointers,
    handle: GLuint
}

impl VertexArrayObject
{
    pub fn new(pointers: &FunctionPointers) -> Self
    {
        let mut handle = 0;
        unsafe{pointers.GenVertexArrays(1, &mut handle)}
        Self{pointers: pointers.clone(), handle}
    }
}

impl Drop for VertexArrayObject
{
    fn drop(&mut self) -> ()
    {
        unsafe{self.pointers.DeleteVertexArrays(1, &self.handle)}
    }
}

impl Deref for VertexArrayObject
{
    type Target = GLuint;
    fn deref(&self) -> &Self::Target
    {
        &self.handle
    }
}

// ----------------------------------------------------------------------

pub struct Shader
{
    pointers: FunctionPointers,
    handle: GLuint
}

impl Shader
{
    pub fn new(pointers: &FunctionPointers, kind: GLenum) -> Self
    {
        Self
        {
            pointers: pointers.clone(),
            handle: unsafe{pointers.CreateShader(kind)}
        }
    }
}

impl Drop for Shader
{
    fn drop(&mut self) -> ()
    {
        unsafe{self.pointers.DeleteShader(self.handle)}
    }
}

impl Deref for Shader
{
    type Target = GLuint;
    fn deref(&self) -> &Self::Target
    {
        &self.handle
    }
}

// ----------------------------------------------------------------------

pub struct Program
{
    pointers: FunctionPointers,
    handle: GLuint
}

impl Program
{
    pub fn new(pointers: &FunctionPointers) -> Self
    {
        Self
        {
            pointers: pointers.clone(), 
            handle: unsafe{pointers.CreateProgram()}
        }
    }
}

impl Drop for Program
{
    fn drop(&mut self) -> ()
    {
        unsafe{self.pointers.DeleteProgram(self.handle)}
    }
}

impl Deref for Program
{
    type Target = GLuint;
    fn deref(&self) -> &Self::Target
    {
        &self.handle
    }
}

// ----------------------------------------------------------------------

pub struct Texture
{
    pointers: FunctionPointers,
    handle: GLuint
}

impl Texture
{
    pub fn new(pointers: &FunctionPointers) -> Self
    {
        let mut handle = 0;
        unsafe{pointers.GenTextures(1, &mut handle)}
        Self{pointers: pointers.clone(), handle}
    }
}

impl Drop for Texture
{
    fn drop(&mut self) -> ()
    {
        unsafe{self.pointers.DeleteTextures(1, &self.handle)}
    }
}

impl Deref for Texture
{
    type Target = GLuint;
    fn deref(&self) -> &Self::Target
    {
        &self.handle
    }
}

// ----------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub enum Error
{
    ShaderCompilation(String),
    ProgramLinking(String),
    AttributeNotFound(String),
    UniformNotFound(String),
    GLError(GLenum)
}

impl std::error::Error for Error {}

impl fmt::Display for Error
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result
    {
        match self
        {
            Self::ShaderCompilation(log) => write!(formatter, "{log}"),
            Self::ProgramLinking(log) => write!(formatter, "{log}"),
            Self::AttributeNotFound(name) => write!
            (
                formatter,
                "Named attribute variable `{name}` is not an active attribute in the specified
                program object or name starts with the reserved prefix `gl_`"
            ),
            Self::UniformNotFound(name) => write!
            (
                formatter, 
                "Uniform `{name}` does not correspond to an active uniform variable in program or
                name is associated with a named uniform block"
            ),
            Self::GLError(flag) => write!
            (
                formatter,
                "{}",
                match *flag
                {
                    INVALID_ENUM => "An unacceptable value is specified for an enumerated argument",
                    INVALID_VALUE => "A numeric argument is out of range",
                    INVALID_OPERATION => "The specified operation is not allowed int he current state",
                    INVALID_FRAMEBUFFER_OPERATION => "The framebuffer object is not complete",
                    OUT_OF_MEMORY => "There is not enough memory left to execute the command",
                    NO_ERROR => "Conflicting error reports",
                    _ => "Unknown OpenGL error"
                }
            )
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

// ----------------------------------------------------------------------------------------------------

pub fn check_for_gl_errors(pointers: &FunctionPointers) -> Result<()>
{
    match unsafe{pointers.GetError()}
    {
        NO_ERROR => Ok(()),
        flag @ _ => Err(Error::GLError(flag))
    }
}

// ----------------------------------------------------------------------------------------------------

pub fn gl_get(pointers: &FunctionPointers, symbol: GLenum) -> GLint
{
    let mut result = 0;
    unsafe{pointers.GetIntegerv(symbol, &mut result)}
    result
}

// ----------------------------------------------------------------------------------------------------

pub fn compile_shader
(
    pointers: &FunctionPointers,
    kind: GLenum,
    code: &str,
) -> Result<Shader>
{
    let mut success = 0;
    let shader = Shader::new(pointers, kind); 
    unsafe
    {
        pointers.ShaderSource(*shader, 1, [code].as_ptr() as _, 0 as _);
        pointers.CompileShader(*shader);
        pointers.GetShaderiv(*shader, COMPILE_STATUS, &mut success)
    }
    match success as GLboolean
    {
        TRUE => Ok(shader),
        _ =>
        {
            let mut log_len: GLint = 0;
            unsafe{pointers.GetShaderiv(*shader, INFO_LOG_LENGTH, &mut log_len)}
            let error_message = match log_len
            {
                0 => String::from("Unknown shader compilation error"),
                _ =>
                {
                    let mut log = vec![0u8; (log_len - 1) as usize];
                    unsafe
                    {
                        pointers.GetShaderInfoLog
                        (
                            *shader, 
                            log_len,
                            0 as _,
                            log.as_mut_ptr() as _
                        )
                    }
                    String::from_utf8(log).unwrap()
                }
            };
            Err(Error::ShaderCompilation(error_message))
        }
    }
}

// ----------------------------------------------------------------------------------------------------

pub fn link_program(pointers: &FunctionPointers, shaders: &[&Shader]) -> Result<Program>
{
    let program = Program::new(pointers);
    let mut success = 0;
    unsafe
    {
        shaders.iter().for_each(|shader| pointers.AttachShader(*program, ***shader));
        pointers.LinkProgram(*program);
        shaders.iter().for_each(|shader| pointers.DetachShader(*program, ***shader));
        pointers.GetProgramiv(*program, LINK_STATUS, &mut success)
    }
    match success as GLboolean
    {
        TRUE => Ok(program),
        _ =>
        {
            let mut log_len: GLint = 0;
            unsafe{pointers.GetProgramiv(*program, INFO_LOG_LENGTH, &mut log_len)}
            let error_message = match log_len
            {
                0 => String::from("Unknown program linking error"),
                _ =>
                {
                    let mut log = vec![0u8; (log_len - 1) as usize];
                    unsafe
                    {
                        pointers.GetProgramInfoLog
                        (
                            *program, 
                            log_len, 
                            0 as _,
                            log.as_mut_ptr() as _
                        )
                    }
                    String::from_utf8(log).unwrap()
                }
            };
            Err(Error::ProgramLinking(error_message))
        }
    }
}

// ----------------------------------------------------------------------------------------------------

pub fn get_attribute_location
(
    pointers: &FunctionPointers,
    program: &Program, 
    name: &str
) -> Result<GLuint>
{
    let cname = CString::new((name).clone()).unwrap();
    unsafe
    {
        let location = pointers.GetAttribLocation(**program, cname.as_ptr());
        match location
        {
            -1 => Err(Error::AttributeNotFound(name.to_string())),
            _ => Ok(location as _)
        }
    }
}

pub fn get_uniform_location
(
    pointers: &FunctionPointers,
    program: &Program,
    name: &str
) -> Result<GLint>
{
    let cname = CString::new((name).clone()).unwrap();
    unsafe
    {
        let location = pointers.GetUniformLocation(**program, cname.as_ptr());
        match location
        {
            -1 => Err(Error::UniformNotFound(name.to_string())),
            _ => Ok(location)
        }
    }
}

// ----------------------------------------------------------------------------------------------------

pub trait UniformDataType
{
    fn to_uniform(&self, pointers: &FunctionPointers, location: GLint) -> ();
}

macro_rules! impl_uniforms
{
    () =>
    {
        impl_uniforms!{@ GLfloat > Uniform1fv}
        impl_uniforms!{@ GLint > Uniform1iv}
        impl_uniforms!{@ GLuint > Uniform1uiv}
        impl_uniforms!{@ GLfloat > Uniform2fv/2, Uniform3fv/3, Uniform4fv/4}
        impl_uniforms!{@ GLint > Uniform2iv/2, Uniform3iv/3, Uniform4iv/4}
        impl_uniforms!{@ GLuint > Uniform2uiv/2, Uniform3uiv/3, Uniform4uiv/4}

    };
    (@ $target:ty > $function:ident) =>
    {
        impl UniformDataType for $target
        {
            fn to_uniform(&self, pointers: &FunctionPointers, location: GLint) -> ()
            {
                unsafe{pointers.$function(location, 1, self)}
            }
        }
    };
    (@ $target:ty > $($function:ident/$size:literal),+) =>
    {$(
        impl UniformDataType for [$target; $size]
        {
            fn to_uniform(&self, pointers: &FunctionPointers, location: GLint) -> ()
            {
                unsafe{pointers.$function(location, 1, self.as_ptr())}
            }
        }
    )+}
}

impl_uniforms!{}

// ----------------------------------------------------------------------------------------------------

fn fill_buffer<T>
(
    pointers: &FunctionPointers, 
    target: GLenum,
    data: &[T],
    usage: GLenum
) -> ()
{
    unsafe
    {
        pointers.BufferData
        (
            target, 
            (data.len() * std::mem::size_of::<T>()) as _,
            data.as_ptr() as _,
            usage
        )
    }
}

// ----------------------------------------------------------------------------------------------------

pub trait AttributeBaseDataType
{
    const TYPE_ENUM: GLenum;
}

impl AttributeBaseDataType for GLubyte
{
    const TYPE_ENUM: GLenum = UNSIGNED_BYTE;
}

impl AttributeBaseDataType for GLbyte
{
    const TYPE_ENUM: GLenum = BYTE;
}

impl AttributeBaseDataType for GLushort
{
    const TYPE_ENUM: GLenum = UNSIGNED_SHORT;
}

impl AttributeBaseDataType for GLshort
{
    const TYPE_ENUM: GLenum = SHORT;
}

impl AttributeBaseDataType for GLuint
{
    const TYPE_ENUM: GLenum = UNSIGNED_INT;
}

impl AttributeBaseDataType for GLint
{
    const TYPE_ENUM: GLenum = INT;
}

impl AttributeBaseDataType for GLfloat
{
    const TYPE_ENUM: GLenum = FLOAT;
}

impl AttributeBaseDataType for GLdouble
{
    const TYPE_ENUM: GLenum = DOUBLE;
}

pub trait Attribute
{
    fn to_attribute
    (
        &self, 
        pointers: &FunctionPointers,
        location: GLuint
    ) -> Result<Buffer>;
}

macro_rules! impl_attributes
{
    () => {impl_attributes!{@ 1, 2, 3, 4}};
    (@ $($components:literal), *) =>
    {$(
        impl<T, const N: usize> Attribute for [[T; $components]; N]
        where 
            T: AttributeBaseDataType + Clone
        {
            fn to_attribute
            (
                &self,
                pointers: &FunctionPointers,
                location: GLuint
            ) -> Result<Buffer>
            {
                unsafe{pointers.GetError()};
                let flattened: Vec<T> = self.clone().into_iter().flatten().collect();
                let vbo = Buffer::new(pointers);
                let previously_bound = gl_get(pointers, ARRAY_BUFFER_BINDING);
                unsafe{pointers.BindBuffer(ARRAY_BUFFER, *vbo)}
                fill_buffer(pointers, ARRAY_BUFFER, &flattened, STATIC_DRAW);
                unsafe
                {
                    pointers.EnableVertexAttribArray(location);
                    pointers.VertexAttribPointer
                    (
                        location,
                        $components,
                        T::TYPE_ENUM,
                        FALSE, 
                        0, 
                        0 as _
                    );
                    pointers.BindBuffer(ARRAY_BUFFER, previously_bound as _);
                }
                check_for_gl_errors(pointers)?;
                Ok(vbo)
            }
        }
    )*}
}

impl_attributes!{}

// ----------------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub enum ChannelCount
{
    One,
    Two,
    Three,
    Four
}

pub struct Image<'data, D>
{
    pub data: Option<&'data Vec<D>>,
    pub resolution: [u32; 2],
    pub channel_count: ChannelCount
}

// ----------------------------------------------------------------------------------------------------

#[non_exhaustive]
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum WrapMode
{
    Repeat,
    MirroredRepeat,
    ClampToEdge
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum InterpolationType
{
    Nearest,
    Linear
}

// ----------------------------------------------------------------------------------------------------

pub trait TextureBaseDataType
{
    const TYPE_ENUM: GLenum;
}

impl TextureBaseDataType for GLubyte
{
    const TYPE_ENUM: GLenum = UNSIGNED_BYTE;
}

impl TextureBaseDataType for GLbyte
{
    const TYPE_ENUM: GLenum = BYTE;
}

impl TextureBaseDataType for GLushort
{
    const TYPE_ENUM: GLenum = UNSIGNED_SHORT;
}

impl TextureBaseDataType for GLshort
{
    const TYPE_ENUM: GLenum = SHORT;
}

impl TextureBaseDataType for GLuint
{
    const TYPE_ENUM: GLenum = UNSIGNED_INT;
}

impl TextureBaseDataType for GLint
{
    const TYPE_ENUM: GLenum = INT;
}

impl TextureBaseDataType for GLfloat
{
    const TYPE_ENUM: GLenum = FLOAT;
}

pub fn create_texture
(
    pointers: &FunctionPointers,
    wrap_mode: Option<WrapMode>,
    minification_filter: InterpolationType,
    magnification_filter: InterpolationType,
    mimap_filter: Option<InterpolationType>
) -> Texture
{
    use InterpolationType::*;
    use WrapMode::*;
    let texture = Texture::new(pointers);
    let previously_bound = gl_get(pointers, TEXTURE_BINDING_2D);
    unsafe
    {
        pointers.BindTexture(TEXTURE_2D, *texture);
        if let Some(wrap_mode) = wrap_mode
        {
            let wrap_mode = match wrap_mode
            {
                Repeat => REPEAT,
                MirroredRepeat => MIRRORED_REPEAT,
                ClampToEdge => CLAMP_TO_EDGE
            };
            pointers.TexParameteri(TEXTURE_2D, TEXTURE_WRAP_S, wrap_mode as _);
            pointers.TexParameteri(TEXTURE_2D, TEXTURE_WRAP_T, wrap_mode as _);
        };
        pointers.TexParameteri
        (
            TEXTURE_2D, 
            TEXTURE_MIN_FILTER, 
            match mimap_filter
            {
                Some(mimap_filter) => 
                {
                    pointers.GenerateMipmap(TEXTURE_2D);
                    match (minification_filter, mimap_filter)
                    {
                        (Nearest, Nearest) => NEAREST_MIPMAP_NEAREST,
                        (Nearest, Linear) => NEAREST_MIPMAP_LINEAR,
                        (Linear, Nearest) => LINEAR_MIPMAP_NEAREST,
                        (Linear, Linear) => LINEAR_MIPMAP_LINEAR
                    }
                },
                None => match minification_filter
                {
                    Nearest => NEAREST,
                    Linear => LINEAR
                }
            } as _
        );
        pointers.TexParameteri
        (
            TEXTURE_2D, 
            TEXTURE_MAG_FILTER, 
            match magnification_filter
            {
                Linear => LINEAR,
                Nearest => NEAREST
            } as _
        );
        pointers.BindTexture(TEXTURE_2D, previously_bound as _)
    }
    texture
}

pub fn fill_texture<T: TextureBaseDataType>
(
    pointers: &FunctionPointers,
    texture: &Texture,
    mimap: bool,
    image: Image<T>
) -> ()
{
    use ChannelCount::*;
    let previously_bound = gl_get(pointers, TEXTURE_BINDING_2D);
    unsafe
    {
        pointers.BindTexture(TEXTURE_2D, **texture);
        pointers.TexImage2D
        (
            TEXTURE_2D,
            0,
            match T::TYPE_ENUM
            {
                UNSIGNED_BYTE | BYTE => RGBA8,
                UNSIGNED_SHORT | SHORT => RGBA16,
                UNSIGNED_INT | INT | FLOAT => RGBA32F,
                _ => unreachable!("Uncovered type")
            } as _,
            image.resolution[0] as _,
            image.resolution[1] as _,
            0,
            match image.channel_count
            {
                One => RED,
                Two => RG,
                Three => RGB,
                Four => RGBA
            },
            T::TYPE_ENUM,
            match image.data
            {
                Some(data) => data.as_ptr() as _,
                None => 0 as _
            }
        );
        if mimap {pointers.GenerateMipmap(TEXTURE_2D)}
        pointers.BindTexture(TEXTURE_2D, previously_bound as _)
    }
}
