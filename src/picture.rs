
use std::{io, fmt, time::*};
use super::ogl;
use image::{ImageFormat::*, codecs::*, GenericImageView, DynamicImage::*, ImageDecoder};

// ----------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub enum ICCError
{
    LCMS2Error(lcms2::Error),
    UnsupportedBitDepth
}

impl std::error::Error for ICCError {}

impl fmt::Display for ICCError
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result
    {
        match self 
        {
            Self::LCMS2Error(error) => write!(formatter, "{}", error),
            Self::UnsupportedBitDepth => write!
            (
                formatter, 
                "Unsupported bit depth for icc transformation"
            )
        }
    }
}

// ----------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub enum PictureError
{
    IO(std::io::Error),
    ImageError(image::error::ImageError),
    ICCError(ICCError),
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
            Self::ImageError(error) => write!(formatter, "{}", error),
            Self::ICCError(error) => write!(formatter, "{}", error),
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
        Self::ICCError(ICCError::LCMS2Error(error))
    }
}

pub type PictureResult<T> = std::result::Result<T, PictureError>;

// ----------------------------------------------------------------------------------------------------

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

// ----------------------------------------------------------------------------------------------------

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

// ----------------------------------------------------------------------------------------------------

#[derive(Clone)]
pub enum PixelData
{
    EightBit(Vec<u8>),
    SixteenBit(Vec<u16>),
    ThirtyTwoBit(Vec<f32>)
}

// ----------------------------------------------------------------------------------------------------

pub struct Still
{
    pub pixel_data: PixelData,
    pub resolution: [u32; 2], // **
    pub channel_count: ogl::ChannelCount,
    pub channel_interpretation: ChannelInterpretation,
    pub gamma: f32,
    pub icc: lcms2::Profile
}

impl TryFrom<(lcms2::Profile, image::DynamicImage)> for Still
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
            ImageRgb32F(buffer) => ThirtyTwoBit(buffer.into_raw()),
            ImageRgba32F(buffer) => ThirtyTwoBit(buffer.into_raw()),
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

impl Still
{
    pub fn apply_icc_transform(&mut self, target: &lcms2::Profile) -> PictureResult<()>
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
            _ => Err(PictureError::ICCError(ICCError::UnsupportedBitDepth))
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

// ----------------------------------------------------------------------------------------------------

pub struct IteratorStasher<I: Iterator>
{
    iterator: I,
    stash: Vec<I::Item>
}

impl<I: Iterator> IteratorStasher<I>
{
    fn new(iterator: I) -> Self
    {
        Self{iterator, stash: vec![]}
    }
    
    fn stash_next(&mut self) -> Option<()>
    {
        self.iterator.next().map
            (|entry| self.stash.push(entry))
    }
}

// ----------------------------------------------------------------------------------------------------

struct IteratorLooper<I: Iterator>
{
    stasher: IteratorStasher<I>,
    playhead: usize
}

impl<I: Iterator> IteratorLooper<I>
{
    fn new(iterator: I) -> PictureResult<Self>
    {
        let mut stasher = IteratorStasher::new(iterator);
        stasher.stash_next()
            .ok_or(PictureError::ZeroFrames)
            .map(|()| Self{stasher, playhead: 0})
    }

    fn advance(&mut self) -> &I::Item
    {
        self.stasher.stash_next();
        let entry = &self.stasher.stash[self.playhead];
        self.playhead = 
            (self.playhead + 1) %
            self.stasher.stash.len();
        entry
    }
}

// ----------------------------------------------------------------------------------------------------

pub struct Sample<D>
{
    pub data: D,
    pub interval: Duration
}

impl From<(lcms2::Profile, image::Frame)> for Sample<Still>
{
    fn from((icc, frame): (lcms2::Profile, image::Frame)) -> Self
    {
        let interval = Duration::from(frame.delay());
        let buffer = frame.into_buffer();
        let resolution = buffer.dimensions();
        let still = Still
        {
            resolution: [resolution.0, resolution.1],
            channel_count: ogl::ChannelCount::Four,
            pixel_data: PixelData::EightBit(buffer.into_raw()),
            channel_interpretation: ChannelInterpretation::RGBA,
            gamma: 1.0,
            icc
        };
        Self{data: still, interval}
    }
}

// ----------------------------------------------------------------------------------------------------

pub struct StreamingPlayer<I: Iterator>
{
    looper: IteratorLooper<I>,
    onset: Instant,
    interval: Duration
}

impl<I, D> StreamingPlayer<I>
where
    I: Iterator<Item = PictureResult<Sample<D>>>
{
    pub fn new(iterator: I) -> PictureResult<Self>
    {
        let this = Self
        {
            looper: IteratorLooper::new(iterator)?,
            onset: Instant::now(),
            interval: Duration::ZERO
        };
        Ok(this)
    }

    pub fn next(&mut self) -> std::result::Result<Option<&D>, &PictureError>
    {
        if self.onset.elapsed() >= self.interval
        {
            return self.looper.advance().as_ref().map
            (
                |sample|
                {
                    self.onset = Instant::now();
                    self.interval = sample.interval;
                    Some(&sample.data)
                }
            )
        }
        Ok(None)
    }
}

// ----------------------------------------------------------------------------------------------------

type Motion = StreamingPlayer
<
    Box<dyn Iterator<Item = PictureResult<Sample<Still>>>>
>;

struct Newtype<T>(T); // E0119

impl<A> TryFrom<(lcms2::Profile, Newtype<A>)> for Motion
where
    A: image::AnimationDecoder<'static>
{
    type Error = PictureError;
    fn try_from((icc, decoder): (lcms2::Profile, Newtype<A>)) -> PictureResult<Self>
    {
        let icc = icc.icc()?;
        let frames = decoder.0.into_frames();
        let samples = frames.map
        (
            move |result| result.map_err(PictureError::ImageError)
                .map
                (
                    |frame| lcms2::Profile::new_icc(&icc)
                        .map_err(PictureError::from)
                        .map(|icc| Sample::from((icc, frame)))
                )
        ).flatten();
        let samples = Box::new(samples) as _;
        let streamer = StreamingPlayer::new(samples)?;
        Ok(streamer)
    }
}

// ----------------------------------------------------------------------------------------------------

pub enum Picture
{
    Still(PictureResult<Still>),
    Motion(Motion)
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
                            .and_then(|d| Still::try_from((icc, d)))
                    ),
                    true =>
                    {
                        let decoder = Newtype(decoder.apng());
                        let motion = Motion::try_from((icc, decoder))?;
                        Self::Motion(motion)
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
                        .and_then(|d| Still::try_from((icc, d)))
                )
            }
            Gif =>
            {
                let reader = reader.into_inner();
                let decoder = gif::GifDecoder::new(reader)
                    .map_err(PictureError::ImageError)?;
                let decoder = Newtype(decoder);
                let motion = Motion::try_from((srgb, decoder))?;
                Self::Motion(motion)
            }
            OpenExr => Self::Still
            (
                reader.decode()
                    .map_err(PictureError::ImageError)
                    .and_then(|d| Still::try_from((srgb, d)))
                    .map
                    (
                        |mut s|
                        {
                            s.gamma = 1.0 / 2.2;
                            s
                        }
                    )
            ),
            _ => Self::Still
            (
                reader.decode()
                    .map_err(PictureError::ImageError)
                    .and_then(|d| Still::try_from((srgb, d)))
            )
        };
        Ok(this)
    }
}

pub fn open(filepath: &std::path::Path) -> PictureResult<Picture>
{
    image::io::Reader::open(filepath).map_err(PictureError::IO)
        .and_then(Picture::try_from)
}

// ----------------------------------------------------------------------------------------------------

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
