//! 配置模块：`aki.toml` 的模型与加载。
//!
//! 一个领域/来源一段配置，各占一个文件：
//!
//! | 文件 | 配置段 | 内容 |
//! |------|--------|------|
//! | [`anime`] | `[anime]` | 番剧数据目录 |
//! | [`game`] | `[game]` | 游戏数据目录 |
//! | [`bangumi`] | `[bangumi]` | Bangumi 原始响应的落盘位置 |
//! | [`steam`] | `[steam]` | Steam 接口配置与原始响应的落盘位置 |

mod anime;
mod bangumi;
mod game;
mod steam;

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

pub use anime::ConfigAnime;
pub use bangumi::ConfigBangumi;
pub use game::ConfigGame;
pub use steam::ConfigSteam;

/// 全局配置单例，首次访问时加载。
pub static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    #[cfg(debug_assertions)]
    {
        // 调试构建默认在 `database/` 下跑；切不过去直接按不可恢复错误退出。
        if let Err(err) = std::env::set_current_dir("aki-data") {
            eprintln!("[x] enter 'database' failed: {err}");
            std::process::exit(1);
        }
    }
    let config = Config::load_with_current_dir();
    match config {
        Ok(config) => config,
        Err(err) => {
            eprintln!("[x] load config failed: {err}");
            std::process::exit(1);
        }
    }
});

#[derive(Debug, thiserror::Error)]
pub enum LoadConfigError {
    #[error("config file 'aki.toml' not found in current directory")]
    FileNotFound,
    #[error("config deserialize error")]
    TomlDeserialize(#[source] toml::de::Error),
    #[error("io error")]
    Io(#[source] io::Error),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    /// 配置文件所在目录（即 `database/`），所有相对路径都相对它解析。
    #[serde(default, skip)]
    pub root: PathBuf,
    /// 系统语言环境，加载时自动检测。
    #[serde(default, skip)]
    pub locale: String,
    pub anime: ConfigAnime,
    pub game: ConfigGame,
    /// Bangumi 配置。
    #[serde(default)]
    pub bangumi: ConfigBangumi,
    /// Steam 配置。
    #[serde(default)]
    pub steam: ConfigSteam,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, LoadConfigError> {
        if !path.exists() {
            return Err(LoadConfigError::FileNotFound);
        }
        let file = File::open(path).map_err(LoadConfigError::Io)?;
        let s = io::read_to_string(file).map_err(LoadConfigError::Io)?;
        let mut config: Config = toml::from_str(&s).map_err(LoadConfigError::TomlDeserialize)?;
        config.root = path
            .parent()
            .expect("Get aki config path's parent failed.")
            .to_path_buf();
        let locale = sys_locale::get_locales()
            .find(|x| x != "C")
            .unwrap_or("C".to_string());
        config.locale = locale;
        Ok(config)
    }

    pub fn load_with_current_dir() -> Result<Self, LoadConfigError> {
        let mut path = std::env::current_dir().map_err(LoadConfigError::Io)?;
        path.push("aki.toml");
        Self::load(&path)
    }
}
