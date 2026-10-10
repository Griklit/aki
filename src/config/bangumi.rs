use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn default_bangumi_api() -> String {
    "api.bgm.tv".to_string()
}

fn default_bangumi_path() -> PathBuf {
    PathBuf::from("source/bangumi")
}

/// `[bangumi]`：Bangumi 接口地址与原始数据的落盘位置。
///
/// 与 [`super::ConfigAnime`] / [`super::ConfigGame`] 平级——Bangumi 是数据来源，
/// 不是某个领域的子模块，所以 `bangumi_api` 也挂在这里而不是 `[anime]` 下。
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigBangumi {
    /// 相对于 [`super::Config::root`] 的目录，`Subject` 原始响应落在它的 `subject/` 下。
    #[serde(default = "default_bangumi_path")]
    pub path: PathBuf,
    /// Bangumi API 的 base 地址，默认 `api.bgm.tv`。
    #[serde(default = "default_bangumi_api")]
    pub bangumi_api: String,
}

impl Default for ConfigBangumi {
    fn default() -> Self {
        Self {
            path: default_bangumi_path(),
            bangumi_api: default_bangumi_api(),
        }
    }
}
