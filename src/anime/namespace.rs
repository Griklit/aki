use crate::{CONFIG, Namespace};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
pub struct AnimeNamespace;

impl Namespace for AnimeNamespace {
    fn new() -> Self {
        AnimeNamespace
    }

    fn kind() -> &'static str {
        "anime"
    }

    fn data_path() -> PathBuf {
        CONFIG.anime_data_path().to_path_buf()
    }
}
