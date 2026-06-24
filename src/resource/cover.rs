use std::path::PathBuf;

use crate::resource::core::FileResource;

use super::{ImageType, Resource};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Cover {
    image_type: ImageType,
    path: PathBuf,
}

impl Resource for Cover {
    type GetResourceError = std::io::Error;

    #[inline]
    fn kind() -> &'static str {
        "cover"
    }

    fn get(&self) -> Result<Vec<u8>, std::io::Error> {
        self.image_type.get(&self.path)
    }
}

impl FileResource for Cover {
    fn file_path(&self) -> &std::path::Path {
        &self.path
    }

    #[inline]
    fn extension(&self) -> &'static str {
        self.image_type.extension()
    }
}
