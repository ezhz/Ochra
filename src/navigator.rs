
use::
{
    std::
    {
        fmt,
        fs,
        io,
        result,
        sync::*,
        path::*,
        time::*
    },
    notify::
    {
        Watcher as _,
        DebouncedEvent::*
    }
};

// ------------------------------------------------------------

#[derive(Debug)]
pub enum NavigatorError
{
    IO(io::Error),
    Notify(notify::Error),
    InvalidPath(PathBuf),
    NoMatchingEntry(PathBuf),
    EmptyList
}

impl std::error::Error for NavigatorError {}

impl fmt::Display for NavigatorError
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result
    {
        match self
        {
            Self::IO(error)
                => write!(formatter, "IO error {error}"),
            Self::Notify(error)
                => write!(formatter, "File watcher error {error}"),
            Self::InvalidPath(path)
                => write!(formatter, "Invalid path {:?}", path),
            Self::NoMatchingEntry(path)
                => write!(formatter, "Unsupported extension {:?}", path),
            Self::EmptyList
                => write!(formatter, "Empty filepaths list")
        }
    }
}

// ------------------------------------------------------------

pub type NavigatorResult<T> = std::result::Result<T, NavigatorError>;

// ------------------------------------------------------------

pub struct Watcher
{
    #[allow(dead_code)]
    watcher: notify::RecommendedWatcher,
    receiver: mpsc::Receiver<notify::DebouncedEvent>
}

impl Watcher
{
    pub fn watch<P: AsRef<Path>>(path: P) -> result::Result<Self, notify::Error>
    {
        let (sender, receiver) = mpsc::channel();
        let mut watcher = notify::watcher(sender, Duration::from_millis(250))?;
        watcher.watch(path, notify::RecursiveMode::NonRecursive)?;
        Ok(Self{watcher, receiver})
    }

    pub fn receive(&self) -> mpsc::TryIter<notify::DebouncedEvent>
    {
        self.receiver.try_iter()
    }
}

// ------------------------------------------------------------

enum FileType
{
    Directory,
    File,
    SymbolicLink,
    Unknown
}

impl <P: AsRef<Path>> From<P> for FileType
{
    fn from(path: P) -> Self
    {
        let path = path.as_ref();
        if path.is_file() { Self::File }
        else if path.is_dir() { Self::Directory } 
        else if path.is_symlink() { Self::SymbolicLink }
        else { Self::Unknown }
    }
}

impl FileType
{
    fn as_dirpath<P: AsRef<Path>>(path: P) -> NavigatorResult<PathBuf>
    {
        let path = path.as_ref().to_owned();
        let directory_path = match Self::from(&path)
        {
            Self::Directory => path,
            Self::File => path.parent().unwrap().to_owned(),
            _ => return Err(NavigatorError::InvalidPath(path))
        };
        Ok(directory_path)
    }
}

// ------------------------------------------------------------

struct Filepaths(Vec<PathBuf>);

impl Filepaths
{
    fn from_path<P: AsRef<Path>>(path: P) -> NavigatorResult<Self>
    {
        let filepaths = 
            fs::read_dir(&FileType::as_dirpath(path)?)
            .map_err(NavigatorError::IO)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .collect();
        Ok(Self(filepaths))
    }
    
    fn search_for<P: AsRef<Path>>(&self, path: P) -> Option<usize>
    {
        self.0.iter().position(|p| p == path.as_ref())
    }
    
    fn filter_by_extensions
    (
        &mut self, 
        list: &Vec<&'static str>
    ) -> ()
    {
        let predicate = |path: &PathBuf| match path.extension()
        {
            Some(extension) => list.iter()
                .any(|x| extension.eq_ignore_ascii_case(x)),
            None => false
        };
        self.0.retain(predicate)
    }
    
    fn sort(&mut self) -> ()
    {
        self.0.sort()
    }
}

// ------------------------------------------------------------

pub struct FilepathsNavigator
{
    filepaths: Filepaths,
    extensions: Vec<&'static str>,
    cursor: usize,
    watcher: Watcher
}

impl FilepathsNavigator
{
    pub fn from_path<P: AsRef<Path>>
    (
        path: P,
        extensions: &Vec<&'static str>
    ) -> NavigatorResult<Self>
    {
        let path = path.as_ref().to_path_buf();
        let mut filepaths = Filepaths::from_path(&path)?;
        filepaths.filter_by_extensions(extensions);
        filepaths.sort();
        let cursor = match path.is_file()
        {
            true => match filepaths.search_for(&path)
            {
                Some(index) => index,
                None => return Err(NavigatorError::NoMatchingEntry(path))
            }
            false => 0
        };
        let extensions = extensions.clone();
        let watcher = Watcher::watch(&FileType::as_dirpath(path)?).map_err(NavigatorError::Notify)?;
        let this = Self{filepaths, extensions, cursor, watcher};
        this.nonempty()?;
        Ok(this)
    }

    pub fn navigate<D>(&mut self, direction: D) -> ()
    where D: Into<i8>
    {
        let len = self.filepaths.0.len();
        self.cursor = (self.cursor as i64 + direction.into() as i64)
            .rem_euclid(len as _) as _
    }

    fn nonempty(&self) -> NavigatorResult<()>
    {
        (!self.filepaths.0.is_empty()).then(|| ())
            .ok_or(NavigatorError::EmptyList)
    }
    
    pub fn selected(&self) -> &PathBuf
    {
        &self.filepaths.0[self.cursor]
    }

    fn rescan(&mut self) -> NavigatorResult<()>
    {
        let selected = self.selected();
        let mut filepaths = Filepaths::from_path(selected)?;
        filepaths.filter_by_extensions(&self.extensions);
        filepaths.sort();
        let cursor = filepaths.search_for(selected)
            .ok_or(NavigatorError::NoMatchingEntry(selected.clone()))?;
        self.filepaths = filepaths;
        self.cursor = cursor;
        self.nonempty()
    }
    
    pub fn refresh(mut self) -> NavigatorResult<(Self, bool)>
    {
        let messages: Vec<notify::DebouncedEvent> =
            self.watcher.receive().collect();
        let (mut dirty, mut rescan) = (false, false);
        for received in messages
        {
            match received
            {
                Write(path) if &path == self.selected()
                    => dirty = true,
                Rescan | Chmod(..) | Create(..) => rescan = true,
                Remove(path) => if let Some(index) =
                    self.filepaths.search_for(&path)
                {
                    self.filepaths.0.remove(index);
                    self.nonempty()?;
                    if index == self.cursor
                    {
                        self.cursor %= self.filepaths.0.len();
                        dirty = true
                    }
                    else if index < self.cursor
                    {
                        self.cursor -= 1
                    } 
                }
                Rename(source, destination)
                    if &source == self.selected() =>
                {
                    self.filepaths.0[self.cursor] = destination;
                    rescan = true
                }
                _ => {}
            }
        }
        if rescan { self.rescan()? }
        Ok((self, dirty))
    }
}
