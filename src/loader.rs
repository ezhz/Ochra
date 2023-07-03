
use
{
    std::
    {
        path::*,
        fmt,
        sync::{Arc, Mutex, mpsc::*}
    },
    super::
    {
        utility::*,
        picture::*,
    }
};

// ----------------------------------------------------------------------------------------------------

struct ThreadedPictureDecoder
{
    send_to_thread_path: Sender<PathBuf>,
    receive_on_main_path: Receiver<PathBuf>,
    send_to_thread_continue: Sender<()>,
    picture_result: Arc<Mutex<Option<PictureResult<Picture>>>>,
    current_path: Option<PathBuf>
}

impl ThreadedPictureDecoder
{
    fn new() -> Self 
    {
        let (send_to_thread_path, receive_on_thread_path)
            : (Sender<PathBuf>, _) = channel();
        let (send_to_main_path, receive_on_main_path)
            : (Sender<PathBuf>, _) = channel();
        let (send_to_thread_continue, receive_on_thread_continue)
            : (Sender<()>, _) = channel();
        let picture_result = Arc::new(Mutex::new(None));
        let picture_result_thread = picture_result.clone();
        std::thread::spawn
        (
            move || loop
            {
                match receive_on_thread_path.try_iter().last()
                {
                    Some(filepath) =>
                    {
                        *picture_result_thread.lock().unwrap()
                            = Some(open_picture(&filepath));
                        send_to_main_path.send(filepath).unwrap();
                        receive_on_thread_continue.recv().unwrap()
                    }
                    None => {}
                }
            }
        );
        Self
        {
            send_to_thread_path,
            receive_on_main_path,
            send_to_thread_continue,
            picture_result,
            current_path: None
        }
    }

    fn set_filepath<P: AsRef<Path>>(&mut self, path: P) -> ()
    {
        let path = path.as_ref().to_owned();
        self.send_to_thread_path.send(path.clone())
            .map_err(|e| show_error_box(&e, true))
            .unwrap();
        self.current_path = Some(path)
    }

    fn try_fetch_picture(&self) -> Option<PictureResult<Picture>>
    {
        let path = self.current_path.as_ref()?;
        match self.receive_on_main_path.try_recv()
        {
            Ok(filepath) =>
            {
                let result = filepath.eq(path).then
                (
                    || self.picture_result
                        .lock().unwrap()
                        .take().unwrap()
                );
                self.send_to_thread_continue
                    .send(()).unwrap();
                result
            }
            Err(TryRecvError::Empty) => None,
            Err(error @ TryRecvError::Disconnected) =>
            {
                show_error_box(&error, true);
                unreachable!() // **
            }
        }
    }
}

// ----------------------------------------------------------------------------------------------------

enum FrameStreamer
{
    Still(Option<StillPicture>),
    Motion(FramesPlayer)
}

impl FrameStreamer
{
    fn next(&mut self) -> Option<PictureResult<StillPicture>>
    {
        match self
        {
            Self::Still(still) => still.take()
                .map(|s| Ok(s)),
            Self::Motion(player) => player.next()
                .map(|s| s.clone())
        }
    }
}

impl From<Picture> for FrameStreamer
{
    fn from(picture: Picture) -> Self
    {
        match picture
        {
            Picture::Still(still)
                => Self::Still(Some(still)),
            Picture::Motion(motion)
                => Self::Motion(motion)    
        }
    }
}

// ----------------------------------------------------------------------------------------------------

enum PictureLoadState
{
    PictureError(Option<PictureError>),
    Loading(Option<PictureDimensions>),
    Loaded(FrameStreamer)
}


impl fmt::Debug for PictureLoadState
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result
    {
        match self
        {
            Self::PictureError(error) => write!
            (
                formatter,
                "PictureLoadState::PictureError({error:?})"
            ),
            Self::Loading(dimensions) => write!
            (
                formatter, 
                "PictureLoadState::Loading({dimensions:?})"
            ),
            Self::Loaded(..) => write!
            (
                formatter,
                "PictureLoadState::Loaded"
            )    
        }
    }
}

impl From<PictureResult<PictureDimensions>> for PictureLoadState
{
    fn from(result: PictureResult<PictureDimensions>) -> Self
    {
        match result.map(|r| Some(r)).map_err(|e| Some(e))
        {
            Ok(dimensions) => Self::Loading(dimensions),
            Err(error) => Self::PictureError(error)
        }    
    }
}

impl From<Picture> for PictureLoadState
{
    fn from(picture: Picture) -> Self
    {
        Self::Loaded(picture.into())
    }
}

// ----------------------------------------------------------------------------------------------------

pub enum PictureLoadResult
{
    PictureError(PictureError),
    Loading(PictureDimensions),
    Loaded(StillPicture)
}

impl fmt::Debug for PictureLoadResult
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result
    {
        match self
        {
            Self::PictureError(error) => write!
            (
                formatter,
                "PictureLoadResult::PictureError({error:?})"
            ),
            Self::Loading(dimensions) => write!
            (
                formatter, 
                "PictureLoadResult::Loading({dimensions:?})"
            ),
            Self::Loaded(..) => write!
            (
                formatter,
                "PictureLoadResult::Loaded"
            )    
        }
    }
}

impl From<PictureError> for PictureLoadResult
{
    fn from(error: PictureError) -> Self
    {
        Self::PictureError(error)
    }
}

impl From<PictureDimensions> for PictureLoadResult
{
    fn from(dimensions: PictureDimensions) -> Self
    {
        Self::Loading(dimensions)
    }
}

impl From<StillPicture> for PictureLoadResult
{
    fn from(still: StillPicture) -> Self
    {
        Self::Loaded(still)
    }
}

impl From<PictureResult<StillPicture>> for PictureLoadResult
{
    fn from(result: PictureResult<StillPicture>) -> Self
    {
        match result
        {
            Ok(still) => PictureLoadResult
                ::Loaded(still),
            Err(error) => PictureLoadResult
                ::PictureError(error)
        }
    }
}

// ----------------------------------------------------------------------------------------------------

pub struct PictureLoader
{
    decoder: ThreadedPictureDecoder,
    picture: Option<PictureLoadState>
}

impl PictureLoader
{
    pub fn new() -> Self
    {
        Self
        {
            decoder: ThreadedPictureDecoder::new(),
            picture: None
        }
    }

    pub fn load<P: AsRef<Path>>(&mut self, path: P) -> ()
    {
        self.decoder.set_filepath(&path);
        self.picture = Some(read_dimensions(&path).into())
    }
}

impl Iterator for PictureLoader
{
    type Item = PictureLoadResult;
    fn next(&mut self) -> Option<Self::Item>
    {
        match &mut self.picture
        {
            Some(state) => match state
            {
                PictureLoadState::PictureError(error)
                    => error.take().map(Into::into),
                PictureLoadState::Loading(dimensions)
                    => match dimensions.take()
                {
                    Some(dimensions) => Some(dimensions.into()),
                    None => match self.decoder.try_fetch_picture()?
                    {
                        Ok(picture) =>
                        {
                            self.picture = Some(picture.into());
                            self.next()
                        }
                        Err(error) => Some(error.into())
                    }
                }
                PictureLoadState::Loaded(streamer) => 
                    streamer.next().map(Into::into)
            }
            None => None
        }
    }
}