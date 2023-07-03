
use
{
    std::path::*,
    super::
    {
        picture::*,
        loader::*,
        navigator::*
    }
};

// ------------------------------------------------------------

pub struct PictureDirectoryReader
{
    navigator: FilepathsNavigator,
    loader: PictureLoader
}

impl PictureDirectoryReader
{
    pub fn new<P: AsRef<Path>>(path: P) -> NavigatorResult<Self>
    {
        FilepathsNavigator::from_path(path, &extensions()).map
        (
            |navigator|
            {
                let mut loader = PictureLoader::new();
                loader.load(navigator.selected());
                Self
                {
                    navigator,
                    loader
                }
            }
        )
    }

    pub fn change_path<P>(mut self, path: P) -> NavigatorResult<Self>
    where P: AsRef<Path>
    {
        FilepathsNavigator::from_path(path, &extensions()).map
        (
            |navigator|
            {
                self.loader.load(navigator.selected());
                self.navigator = navigator;
                self
            }
        )
    }

    pub fn selected_filepath(&self) -> &PathBuf
    {
        self.navigator.selected()
    }

    pub fn refresh_filepaths(mut self) -> NavigatorResult<Self>
    {
        let (navigator, dirty) = self.navigator.refresh()?;
        self.navigator = navigator;
        if dirty
        {
            self.loader.load(self.navigator.selected())
        }
        Ok(self)
    }

    pub fn navigate(&mut self, direction: i8) -> ()
    {
        self.navigator.navigate(direction);
        self.loader.load(self.navigator.selected())
    }
}

impl Iterator for PictureDirectoryReader
{
    type Item = PictureLoadResult;
    fn next(&mut self) -> Option<Self::Item>
    {
        self.loader.next()
    }
}
