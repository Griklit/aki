use super::ImageType;
use serde::{Deserialize, Serialize};

pub trait ResourceType {
    fn kind() -> &'static str;
    fn extension(&self) -> &'static str;
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Cover(ImageType);

impl ResourceType for Cover {
    #[inline]
    fn kind() -> &'static str {
        "cover"
    }

    #[inline]
    fn extension(&self) -> &'static str {
        self.0.extension()
    }
}
