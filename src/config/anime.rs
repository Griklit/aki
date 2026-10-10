use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// `[anime]`：番剧数据目录。
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigAnime {
    /// 相对于 [`super::Config::root`] 的目录（`AnimeNamespace::data_path()` 用它）。
    pub path: PathBuf,
}
