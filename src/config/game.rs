use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// `[game]`：游戏数据目录。
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigGame {
    /// 相对于 [`super::Config::root`] 的目录。
    pub path: PathBuf,
}
