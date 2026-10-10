use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    #[cfg(debug_assertions)]
    {
        // 调试构建默认在 `database/` 下跑；切不过去直接按不可恢复错误退出。
        if let Err(err) = std::env::set_current_dir("database") {
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

fn default_bangimi_api() -> String {
    "api.bgm.tv".to_string()
}

fn default_steam_api() -> String {
    "https://api.steampowered.com".to_string()
}

fn default_steam_store_api() -> String {
    "https://store.steampowered.com".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigAnime {
    pub path: PathBuf,
    #[serde(default = "default_bangimi_api")]
    pub bangumi_api: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigGame {
    pub path: PathBuf,
}

/// Bangumi 原始数据的落盘位置。
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigBangumi {
    /// 相对于 [`Config::root`] 的目录，`Subject` 原始响应落在它的 `subject/` 下。
    #[serde(default = "default_bangumi_path")]
    pub path: PathBuf,
}

fn default_bangumi_path() -> PathBuf {
    PathBuf::from("source/bangumi")
}

impl Default for ConfigBangumi {
    fn default() -> Self {
        Self {
            path: default_bangumi_path(),
        }
    }
}

/// Steam 接口配置与原始数据的落盘位置。
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigSteam {
    /// 相对于 [`Config::root`] 的目录，原始响应就落在它下面。
    #[serde(default = "default_steam_path")]
    pub path: PathBuf,
    /// Steam Web API 密钥。留空则不发送，需要密钥的接口会直接报错。
    #[serde(default)]
    pub api_key: String,
    /// SteamID64，用于拉取「已拥有游戏」「最近游玩」等玩家数据。
    #[serde(default)]
    pub steam_id: Option<String>,
    /// 商店接口的响应语言，如 `schinese`。
    #[serde(default = "default_steam_language")]
    pub language: String,
    /// 商店接口的地区，决定价格与分区，如 `cn`。
    #[serde(default = "default_steam_country")]
    pub country: String,
    #[serde(default = "default_steam_api")]
    pub api: String,
    #[serde(default = "default_steam_store_api")]
    pub store_api: String,
}

fn default_steam_path() -> PathBuf {
    PathBuf::from("source/steam")
}

fn default_steam_language() -> String {
    "schinese".to_string()
}

fn default_steam_country() -> String {
    "cn".to_string()
}

impl Default for ConfigSteam {
    fn default() -> Self {
        Self {
            path: default_steam_path(),
            api_key: String::new(),
            steam_id: None,
            language: default_steam_language(),
            country: default_steam_country(),
            api: default_steam_api(),
            store_api: default_steam_store_api(),
        }
    }
}

/// 数据来源（Bangumi / Steam）的落盘位置与接口配置。
///
/// 这不是某个领域的子模块：`anime` 与 `game` 都只是这些来源的消费方。
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ConfigSource {
    #[serde(default)]
    pub bangumi: ConfigBangumi,
    #[serde(default)]
    pub steam: ConfigSteam,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default, skip)]
    pub root: PathBuf,
    #[serde(default, skip)]
    pub locale: String,
    pub anime: ConfigAnime,
    pub game: ConfigGame,
    #[serde(default)]
    pub source: ConfigSource,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Namespace;

    /// 当前工作目录是进程级的：改它的测试必须串行执行、并在结束（含 panic）后还原，
    /// 否则 `load_config_with_work_dir` 与 `load_config_with_work_dir_panic` 会互相
    /// 把对方的目录改掉。
    ///
    /// 另外工作目录也不可假定：别的测试会惰性初始化 [`CONFIG`]，那一步本身就会
    /// `set_current_dir("database")`。因此这里一律用 `CARGO_MANIFEST_DIR` 拼绝对路径。
    static WORK_DIR: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// 把工作目录切到 `path`，Drop 时还原（panic 展开也会跑到）。
    struct WorkDir(std::path::PathBuf);

    impl WorkDir {
        fn switch_to(path: impl AsRef<Path>) -> Self {
            let path = path.as_ref();
            let original = std::env::current_dir().unwrap();
            std::env::set_current_dir(path).unwrap_or_else(|error| {
                panic!(
                    "switch_to({}) from {} failed: {error}",
                    path.display(),
                    original.display()
                )
            });
            Self(original)
        }
    }

    impl Drop for WorkDir {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }

    /// 临时目录，Drop 时删掉。
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("aki-{name}-{}", uuid::Uuid::now_v7()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn load_config_with_work_dir() {
        let _busy = WORK_DIR
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _work_dir = WorkDir::switch_to(Path::new(env!("CARGO_MANIFEST_DIR")).join("database"));
        let config = Config::load_with_current_dir().unwrap();
        dbg!(&config);
        // 路径不再挂在 `Config` 上，由各命名空间从 `root` 自己算。
        dbg!(crate::anime::AnimeNamespace::data_path());
    }

    /// 空目录里没有 `aki.toml`：`Config::load_with_current_dir` 应当报
    /// [`LoadConfigError::FileNotFound`]（`unwrap` 的 panic 消息里带的是 `Debug`，
    /// 所以这里断言的是变体名而不是 `Display` 文案）。
    ///
    /// 用真实存在的空目录，而不是不存在的路径——`set_current_dir` 自己也会 panic，
    /// 那样测的就不是配置加载了。
    #[test]
    #[should_panic(expected = "FileNotFound")]
    fn load_config_with_work_dir_panic() {
        let empty = TempDir::new("empty-config-dir");
        let _busy = WORK_DIR
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _work_dir = WorkDir::switch_to(&empty.0);
        let _ = Config::load_with_current_dir().unwrap();
    }

    #[test]
    #[ignore = "cannot panic: only prints the lazily loaded config"]
    fn lazy_config() {
        // 同样要先把工作目录摆正：`CONFIG` 是惰性初始化的，而它找不到 `aki.toml`
        // 时会直接 `process::exit(1)`，所以这个测试不能依赖别的测试先碰过 `CONFIG`。
        let _busy = WORK_DIR
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _work_dir = WorkDir::switch_to(Path::new(env!("CARGO_MANIFEST_DIR")).join("database"));
        dbg!(&*CONFIG);
    }
}
