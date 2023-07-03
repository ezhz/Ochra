
use
{
    std::{io, fmt, time::*},
    super::ogl,
    image::
    {
        ImageFormat::*,
        codecs::*,
        GenericImageView,
        DynamicImage::*,
        ImageDecoder
    }
};

// ------------------------------------------------------------

#[derive(Debug)]
pub enum PictureError
{
    IO(std::io::Error),
    ImageError(image::error::ImageError),
    ICCError(lcms2::Error),
    UnsupportedChannelCount(u8),
    UnsupportedImageFormat,
    UnsupportedPixelFormat,
    ZeroFrames
}

impl std::error::Error for PictureError {}

impl fmt::Display for PictureError
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result
    {
        match self 
        {
            Self::IO(error) => write!(formatter, "{}", error),
            Self::ImageError(error) => write!(formatter, "{error}"),
            Self::ICCError(error) => write!(formatter, "ICC error: {error}"),
            Self::UnsupportedChannelCount(count)
                => write!(formatter, "Unsupported channel count {count}"),
            Self::UnsupportedImageFormat
                => write!(formatter, "Unsupported image format"),
            Self::UnsupportedPixelFormat
                => write!(formatter, "Unsupported pixel format"),
            Self::ZeroFrames
                => write!(formatter, "Animated image has no frames")
        }
    }
}

impl From<lcms2::Error> for PictureError
{
    fn from(error: lcms2::Error) -> Self
    {
        Self::ICCError(error)
    }
}

pub type PictureResult<T> = std::result::Result<T, PictureError>;

// ------------------------------------------------------------

impl TryFrom<u8> for ogl::ChannelCount
{
    type Error = PictureError;
    fn try_from(number: u8) -> PictureResult<Self>
    {
        match number
        {
            1 => Ok(Self::One),
            2 => Ok(Self::Two),
            3 => Ok(Self::Three),
            4 => Ok(Self::Four),
            _ => Err(PictureError::UnsupportedChannelCount(number))
        }
    }
}

// ------------------------------------------------------------

#[derive(Clone, Copy)]
pub enum ChannelInterpretation
{
    L,
    LA,
    RGB,
    RGBA
}

impl ChannelInterpretation
{
    pub fn swizzle_for_rgba(&self) -> [i32; 4]
    {
        match self
        {
            Self::L => [0, 0, 0, 3],
            Self::LA => [0, 0, 0, 1],
            Self::RGB => [0, 1, 2, 3],
            Self::RGBA => [0, 1, 2, 3]
        }
    }
}

// ------------------------------------------------------------

#[derive(Clone)]
pub enum PixelData
{
    EightBit(Vec<u8>),
    SixteenBit(Vec<u16>)
}

// ------------------------------------------------------------

pub struct StillPicture
{
    pub pixel_data: PixelData,
    pub resolution: PictureDimensions,
    pub channel_count: ogl::ChannelCount,
    pub channel_interpretation: ChannelInterpretation,
    pub gamma: f32,
    pub icc: lcms2::Profile
}

impl TryFrom<(lcms2::Profile, image::DynamicImage)> for StillPicture
{
    type Error = PictureError;
    fn try_from((icc, dynamic_image): (lcms2::Profile, image::DynamicImage)) -> PictureResult<Self>
    {
        use ogl::ChannelCount::*;
        use ChannelInterpretation::*;
        use PixelData::*;
        let resolution = dynamic_image.dimensions();
        let resolution = [resolution.0, resolution.1];
        let color_type = dynamic_image.color();
        let channel_count = color_type.channel_count();
        let channel_count = ogl::ChannelCount::try_from(channel_count)?;
        let channel_interpretation = match 
        (
            color_type.has_color(),
            color_type.has_alpha(),
            channel_count
        )
        {
            (true, true, Four) => RGBA,
            (true, false, Three) => RGB,
            (false, true, Two) => LA,
            (false, false, One) => L,
            _ => return Err(PictureError::UnsupportedPixelFormat)
        };
        let pixel_data = match dynamic_image
        {
            ImageLuma8(buffer) => EightBit(buffer.into_raw()),
            ImageLumaA8(buffer) => EightBit(buffer.into_raw()),
            ImageRgb8(buffer) => EightBit(buffer.into_raw()),
            ImageRgba8(buffer) => EightBit(buffer.into_raw()),
            ImageLuma16(buffer) => SixteenBit(buffer.into_raw()),
            ImageLumaA16(buffer) => SixteenBit(buffer.into_raw()),
            ImageRgb16(buffer) => SixteenBit(buffer.into_raw()),
            ImageRgba16(buffer) => SixteenBit(buffer.into_raw()),
            ImageRgb32F(_) => SixteenBit(dynamic_image.into_rgb16().into_raw()),
            ImageRgba32F(_) => SixteenBit(dynamic_image.into_rgba16().into_raw()),
            _ => return Err(PictureError::UnsupportedPixelFormat)
        };
        let this = Self
        {
            pixel_data, 
            resolution, 
            channel_count, 
            channel_interpretation,
            gamma: 1.0,
            icc
        };
        Ok(this)
    }
}

impl StillPicture
{
    pub fn transform_to_icc(&mut self, target: &lcms2::Profile) -> PictureResult<()>
    {
        use lcms2::PixelFormat;
        let intent = lcms2::Intent::Perceptual;
        match &mut self.pixel_data
        {
            PixelData::EightBit(pixels_data) =>
            {
                match self.channel_interpretation
                {
                    ChannelInterpretation::L =>
                    {
                        let format = PixelFormat::GRAY_8;
                        let mut pixels = pixels_data.clone();
                        lcms2::Transform::new(&self.icc, format, target, format, intent)
                            .map(|t| t.transform_in_place(&mut pixels))?;
                        Ok(*pixels_data = pixels.into_iter().collect())
                    }
                    ChannelInterpretation::LA =>
                    {
                        let format = PixelFormat::GRAYA_8;
                        let mut pixels = pixels_data.chunks(2)
                            .map(|c| [c[0], c[1]])
                            .collect::<Vec<[u8; 2]>>();
                        lcms2::Transform::new(&self.icc, format, target, format, intent)
                            .map(|t| t.transform_in_place(&mut pixels))?;
                        Ok(*pixels_data = pixels.into_iter().flatten().collect())
                    }
                    ChannelInterpretation::RGB =>
                    {
                        let format = PixelFormat::RGB_8;
                        let mut pixels = pixels_data.chunks(3)
                            .map(|c| [c[0], c[1], c[2]])
                            .collect::<Vec<[u8; 3]>>();
                        lcms2::Transform::new(&self.icc, format, target, format, intent)
                            .map(|t| t.transform_in_place(&mut pixels))?;
                        Ok(*pixels_data = pixels.into_iter().flatten().collect())
                    }
                    ChannelInterpretation::RGBA =>
                    {
                        let format = PixelFormat::RGBA_8;
                        let mut pixels = pixels_data.chunks(4)
                            .map(|c| [c[0], c[1], c[2], c[3]])
                            .collect::<Vec<[u8; 4]>>();
                        lcms2::Transform::new(&self.icc, format, target, format, intent)
                            .map(|t| t.transform_in_place(&mut pixels))?;
                        Ok(*pixels_data = pixels.into_iter().flatten().collect())
                    }
                }
            }
            PixelData::SixteenBit(pixels_data) =>
            {
                match self.channel_interpretation
                {
                    ChannelInterpretation::L =>
                    {
                        let format = PixelFormat::GRAY_16;
                        let mut pixels = pixels_data.clone();
                        lcms2::Transform::new(&self.icc, format, target, format, intent)
                            .map(|t| t.transform_in_place(&mut pixels))?;
                        Ok(*pixels_data = pixels.into_iter().collect())
                    }
                    ChannelInterpretation::LA =>
                    {
                        let format = PixelFormat::GRAYA_16;
                        let mut pixels = pixels_data.chunks(2)
                            .map(|c| [c[0], c[1]])
                            .collect::<Vec<[u16; 2]>>();
                        lcms2::Transform::new(&self.icc, format, target, format, intent)
                            .map(|t| t.transform_in_place(&mut pixels))?;
                        Ok(*pixels_data = pixels.into_iter().flatten().collect())
                    }
                    ChannelInterpretation::RGB =>
                    {
                        let format = PixelFormat::RGB_16;
                        let mut pixels = pixels_data.chunks(3)
                            .map(|c| [c[0], c[1], c[2]])
                            .collect::<Vec<[u16; 3]>>();
                        lcms2::Transform::new(&self.icc, format, target, format, intent)
                            .map(|t| t.transform_in_place(&mut pixels))?;
                        Ok(*pixels_data = pixels.into_iter().flatten().collect())
                    }
                    ChannelInterpretation::RGBA =>
                    {
                        let format = PixelFormat::RGBA_16;
                        let mut pixels = pixels_data.chunks(4)
                            .map(|c| [c[0], c[1], c[2], c[3]])
                            .collect::<Vec<[u16; 4]>>();
                        lcms2::Transform::new(&self.icc, format, target, format, intent)
                            .map(|t| t.transform_in_place(&mut pixels))?;
                        Ok(*pixels_data = pixels.into_iter().flatten().collect())
                    }
                }
            }
        }
    }

    pub fn clone(&self) -> PictureResult<Self>
    {
        Ok
        (
            Self
            {
                pixel_data: self.pixel_data.clone(),
                resolution: self.resolution,
                channel_count: self.channel_count,
                channel_interpretation: self.channel_interpretation,
                gamma: self.gamma,
                icc: lcms2::Profile::new_icc(&self.icc.icc()?)?
            }
        )
    }
}

// ------------------------------------------------------------

pub struct Frame
{
    pub still: StillPicture,
    pub interval: Duration
}

impl From<(lcms2::Profile, image::Frame)> for Frame
{
    fn from((icc, frame): (lcms2::Profile, image::Frame)) -> Self
    {
        let interval = Duration::from(frame.delay());
        let buffer = frame.into_buffer();
        let resolution = buffer.dimensions();
        let still = StillPicture
        {
            resolution: [resolution.0, resolution.1],
            channel_count: ogl::ChannelCount::Four,
            pixel_data: PixelData::EightBit(buffer.into_raw()),
            channel_interpretation: ChannelInterpretation::RGBA,
            gamma: 1.0,
            icc
        };
        Self{still, interval}
    }
}

// ------------------------------------------------------------

pub struct FramesPlayer
{
    frames: Vec<Frame>,
    playhead: usize,
    onset: Instant,
    interval: Duration
}

impl FramesPlayer
{
    fn new(frames: Vec<Frame>) -> PictureResult<Self>
    {
        Ok
        (
            Self
            {
                frames: match frames.len() == 0
                {
                    true => return Err(PictureError::ZeroFrames),
                    false => frames
                },
                playhead: 0,
                onset: Instant::now(),
                interval: Duration::ZERO
            }
        )
    }

    pub fn next(&mut self) -> Option<&StillPicture>
    {
        if self.onset.elapsed() >= self.interval
        {
            let entry = &self.frames[self.playhead];
            self.playhead = (self.playhead + 1) % self.frames.len();
            self.onset = Instant::now();
            self.interval = entry.interval;
            return Some(&entry.still)
        }
        None
    }
}

// ------------------------------------------------------------

struct Newtype<T>(T); // E0119

impl<A> TryFrom<(lcms2::Profile, Newtype<A>)> for FramesPlayer
where
    A: image::AnimationDecoder<'static>
{
    type Error = PictureError;
    fn try_from((icc, decoder): (lcms2::Profile, Newtype<A>)) -> PictureResult<Self>
    {
        let icc = icc.icc()?;
        let frames = decoder.0.into_frames().map
        (
            move |result| result.map_err(PictureError::ImageError)
                .map
                (
                    |frame| lcms2::Profile::new_icc(&icc)
                        .map_err(PictureError::from)
                        .map(|icc| Frame::from((icc, frame)))
                )
        ).flatten().collect::<Result<Vec<_>, _>>()?;
        FramesPlayer::new(frames)
    }
}

// ------------------------------------------------------------

pub enum Picture
{
    Still(StillPicture),
    Motion(FramesPlayer)
}

impl<R> TryFrom<image::io::Reader<R>> for Picture
where
    R: io::Read + io::BufRead + io::Seek + 'static
{
    type Error = PictureError;
    fn try_from(reader: image::io::Reader<R>) -> PictureResult<Self>
    {
        let format = reader.format().ok_or(PictureError::UnsupportedImageFormat)?;
        let srgb = lcms2::Profile::new_srgb();
        let this = match format
        {
            Png =>
            {
                let reader = reader.into_inner();
                let mut decoder = png::PngDecoder::new(reader)
                    .map_err(PictureError::ImageError)?;
                let icc = decoder.icc_profile().map_or
                (
                    Ok(srgb),
                    |icc| lcms2::Profile::new_icc(&icc)
                )?;
                match decoder.is_apng()
                {
                    false => Self::Still
                    (
                        image::DynamicImage::from_decoder(decoder)
                            .map_err(PictureError::ImageError)
                            .and_then(|d| StillPicture::try_from((icc, d)))?
                    ),
                    true =>
                    {
                        let decoder = Newtype(decoder.apng());
                        let player = FramesPlayer::try_from((icc, decoder))?;
                        Self::Motion(player)
                    }
                }
            }
            Jpeg =>
            {
                let reader = reader.into_inner();
                let mut decoder = jpeg::JpegDecoder::new(reader)
                    .map_err(PictureError::ImageError)?;
                let icc = decoder.icc_profile().map_or
                (
                    Ok(srgb),
                    |icc| lcms2::Profile::new_icc(&icc)
                )?;
                Self::Still
                (
                    image::DynamicImage::from_decoder(decoder)
                        .map_err(PictureError::ImageError)
                        .and_then(|d| StillPicture::try_from((icc, d)))?
                )
            }
            Gif =>
            {
                let reader = reader.into_inner();
                let decoder = gif::GifDecoder::new(reader)
                    .map_err(PictureError::ImageError)?;
                let decoder = Newtype(decoder);
                let player = FramesPlayer::try_from((srgb, decoder))?;
                Self::Motion(player)
            }
            OpenExr => Self::Still
            (
                reader.decode()
                    .map_err(PictureError::ImageError)
                    .and_then(|d| StillPicture::try_from((srgb, d)))
                    .map
                    (
                        |mut s|
                        {
                            s.gamma = 1.0 / 2.2;
                            s
                        }
                    )?
            ),
            _ => Self::Still
            (
                reader.decode()
                    .map_err(PictureError::ImageError)
                    .and_then(|d| StillPicture::try_from((srgb, d)))?
            )
        };
        Ok(this)
    }
}

pub fn open_picture(filepath: &std::path::Path) -> PictureResult<Picture>
{
    image::io::Reader::open(filepath).map_err(PictureError::IO)
        .and_then(Picture::try_from)
}

// ------------------------------------------------------------

pub type PictureDimensions = [u32; 2];

// ------------------------------------------------------------

pub fn read_dimensions<P: AsRef<std::path::Path>>(filepath: P)
    -> PictureResult<PictureDimensions>
{
    image::image_dimensions(filepath)
        .map(|(w, h)| [w, h])
        .map_err(PictureError::ImageError)
}

// ------------------------------------------------------------

pub fn extensions() -> Vec<&'static str>
{
    macro_rules! collect_extensions
    {
        ($($format:ident),+) =>
        {{
            let mut extensions: Vec<&str> = vec![];
            $(extensions.extend($format.extensions_str());)+
            extensions
        }}
    }
    collect_extensions!
    [
        Png,
        Jpeg,
        Gif,
        WebP,
        Pnm,
        Tiff,
        Tga,
        Dds,
        Bmp,
        Ico,
        Hdr,
        OpenExr,
        Farbfeld,
        Avif
    ]
}
