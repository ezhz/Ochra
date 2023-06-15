
use std::{io, fmt, time::*};
use super::ogl;
use image::{ImageFormat::*, codecs::*, GenericImageView, DynamicImage::*};

// ----------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub enum Error
{
    IO(std::io::Error),
    ImageError(image::error::ImageError),
    UnsupportedChannelCount(u8),
    UnsupportedImageFormat,
    UnsupportedPixelFormat,
    ZeroFrames
}

impl std::error::Error for Error {}

impl fmt::Display for Error
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result
    {
        match self 
        {
            Self::IO(error) => write!(formatter, "{}", error),
            Self::ImageError(error) => write!(formatter, "{}", error),
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

pub type Result<T> = std::result::Result<T, Error>;

// ----------------------------------------------------------------------------------------------------

impl TryFrom<u8> for ogl::ChannelCount
{
    type Error = Error;
    fn try_from(number: u8) -> Result<Self>
    {
        match number
        {
            1 => Ok(Self::One),
            2 => Ok(Self::Two),
            3 => Ok(Self::Three),
            4 => Ok(Self::Four),
            _ => Err(Error::UnsupportedChannelCount(number))
        }
    }
}

// ----------------------------------------------------------------------------------------------------

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
    pub channel_interpretation: ChannelInterpretation
}

impl TryFrom<image::DynamicImage> for Still
{
    type Error = Error;
    fn try_from(dynamic_image: image::DynamicImage) -> Result<Self>
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
            _ => return Err(Error::UnsupportedPixelFormat)
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
            _ => return Err(Error::UnsupportedPixelFormat)
        };
        let this = Self
        {
            pixel_data, 
            resolution, 
            channel_count, 
            channel_interpretation
        };
        Ok(this)
    }
}

// ----------------------------------------------------------------------------------------------------

pub struct IteratorStasher<I: Iterator>
{
    iterator: I,
    stash: Vec<I::Item>,
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
    fn new(iterator: I) -> Result<Self>
    {
        let mut stasher = IteratorStasher::new(iterator);
        stasher.stash_next()
            .ok_or(Error::ZeroFrames)
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

impl From<image::Frame> for Sample<Still>
{
    fn from(frame: image::Frame) -> Self
    {
        let interval = Duration::from(frame.delay());
        let buffer = frame.into_buffer();
        let resolution = buffer.dimensions();
        let still = Still
        {
            resolution: [resolution.0, resolution.1],
            channel_count: ogl::ChannelCount::Four,
            pixel_data: PixelData::EightBit(buffer.into_raw()),
            channel_interpretation: ChannelInterpretation::RGBA
            
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
    I: Iterator<Item = Result<Sample<D>>>
{
    pub fn new(iterator: I) -> Result<Self>
    {
        let this = Self
        {
            looper: IteratorLooper::new(iterator)?,
            onset: Instant::now(),
            interval: Duration::ZERO
        };
        Ok(this)
    }

    pub fn next(&mut self) -> std::result::Result<Option<&D>, &Error>
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
    Box<dyn Iterator<Item = Result<Sample<Still>>>>
>;

struct Newtype<T>(T); // E0119

impl<A> TryFrom<Newtype<A>> for Motion
where
    A: image::AnimationDecoder<'static>
{
    type Error = Error;
    fn try_from(decoder: Newtype<A>) -> Result<Self>
    {
        let frames = decoder.0.into_frames();
        let samples = frames.map
        (
            |result| result.map_err(Error::ImageError)
                .map(Sample::from)
        );
        let samples = Box::new(samples) as _;
        let streamer = StreamingPlayer::new(samples)?;
        Ok(streamer)
    }
}

// ----------------------------------------------------------------------------------------------------

pub enum Picture
{
    Still(Result<Still>),
    Motion(Motion)
}

impl<R> TryFrom<image::io::Reader<R>> for Picture
where
    R: io::Read + io::BufRead + io::Seek + 'static
{
    type Error = Error;
    fn try_from(reader: image::io::Reader<R>) -> Result<Self>
    {
        let format = reader.format()
            .ok_or(Error::UnsupportedImageFormat)?;
        let this = match format
        {          
            Png =>
            {
                let reader = reader.into_inner();
                let decoder = png::PngDecoder::new(reader)
                    .map_err(Error::ImageError)?;                
                match decoder.is_apng()
                {
                    false => Self::Still
                    (
                        image::DynamicImage::from_decoder(decoder)
                            .map_err(Error::ImageError)
                            .and_then(Still::try_from)
                    ),
                    true =>
                    {
                        let decoder = Newtype(decoder.apng());
                        let motion = Motion::try_from(decoder)?;
                        Self::Motion(motion)
                    }
                }
            }
            Gif =>
            {
                let reader = reader.into_inner();
                let decoder = gif::GifDecoder::new(reader)
                    .map_err(Error::ImageError)?;
                let decoder = Newtype(decoder);
                let motion = Motion::try_from(decoder)?;
                Self::Motion(motion)
            }
            _ => Self::Still
            (
                reader.decode()
                    .map_err(Error::ImageError)
                    .and_then(Still::try_from)
            )
        };
        Ok(this)
    }
}

pub fn open(filepath: &std::path::Path) -> Result<Picture>
{
    image::io::Reader::open(filepath).map_err(Error::IO)
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
