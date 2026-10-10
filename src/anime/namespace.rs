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
    /// fjall 索引数据库目录：`{data_path}/.aki/fjall-index`。
    ///
    /// 索引是番剧领域自己的缓存，所以路径也由这个命名空间给。
    /// 分两段 `join`，这样在 Windows 上打印出来也是 `\` 分隔（这个路径会进日志）。
    #[must_use]
    pub fn index_path() -> PathBuf {
        Self::data_path().join(".aki").join("fjall-index")
    }
}
