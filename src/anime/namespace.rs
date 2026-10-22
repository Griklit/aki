use crate::{CONFIG, Namespace, utils};
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
        utils::path::normalize(CONFIG.root.join(&CONFIG.anime.path))
    }
}

impl AnimeNamespace {
    /// SQLite 索引数据库文件：`{data_path}/.aki/anime-index.sqlite`。
    #[must_use]
    pub fn sqlite_index_path() -> PathBuf {
        Self::data_path().join(".aki").join("anime-index.sqlite")
    }
}
