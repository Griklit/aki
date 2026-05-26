use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum ImageType {
    Png,
    Jpeg,
    Gif,
    Webp,
    Avif,
}

impl ImageType {
    #[must_use]
    pub const fn extension(&self) -> &'static str {
        match self {
            ImageType::Png => "png",
            ImageType::Jpeg => "jpg",
            ImageType::Gif => "gif",
            ImageType::Webp => "webp",
            ImageType::Avif => "avif",
        }
    }

    pub fn from_extension(extension: &str) -> Result<Self, String> {
        let mut extension = extension;
        if extension.starts_with('.') {
            extension = extension.trim_start_matches('.');
        }
        match extension {
            "png" => Ok(ImageType::Png),
            "jpg" | "jpeg" => Ok(ImageType::Jpeg),
            "gif" => Ok(ImageType::Gif),
            "webp" => Ok(ImageType::Webp),
            "avif" => Ok(ImageType::Avif),
            _ => Err("Invalid image type".to_string()),
        }
    }

    #[must_use]
    pub const fn mime_type(&self) -> &'static str {
        match self {
            ImageType::Png => "image/png",
            ImageType::Jpeg => "image/jpeg",
            ImageType::Gif => "image/gif",
            ImageType::Webp => "image/webp",
            ImageType::Avif => "image/avif",
        }
    }
}
