use std::io::Read;
use std::path::Path;
use std::{fs, io};

use crate::CONFIG;

pub trait Resource {
    type GetResourceError: std::error::Error;

    fn kind() -> &'static str;

    fn get(&self) -> Result<Vec<u8>, Self::GetResourceError>;
}

pub trait FileResource {
    fn file_path(&self) -> &Path;

    fn extension(&self) -> &'static str;

    fn load(&self) -> Result<Vec<u8>, io::Error> {
        let mut f = fs::File::open(CONFIG.root.join(self.file_path()))?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        Ok(buf)
    }
}
