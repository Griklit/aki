use crate::utils;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::sync::OnceLock;

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    #[cfg(debug_assertions)]
    {
        std::env::set_current_dir("database").unwrap();
    }
    let config = Config::load_with_current_dir();
    match config {
        Ok(config) => config,
        Err(err) => {
            eprintln!("Load config failed: {err}");
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
    #[serde(default)]
    pub steam: ConfigSteam,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigSteam {
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

fn default_steam_language() -> String {
    "schinese".to_string()
}

fn default_steam_country() -> String {
    "cn".to_string()
}

impl Default for ConfigSteam {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            steam_id: None,
            language: default_steam_language(),
            country: default_steam_country(),
            api: default_steam_api(),
            store_api: default_steam_store_api(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default, skip)]
    pub root: PathBuf,
    #[serde(default, skip)]
    pub locale: String,
    pub anime: ConfigAnime,
    pub game: ConfigGame,
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

    pub fn anime_data_path(&self) -> &'static Path {
        static PATH: OnceLock<PathBuf> = OnceLock::new();
        PATH.get_or_init(|| utils::path::normalize(self.root.join(&self.anime.path)))
    }

    /// 番剧索引数据库目录（fjall），见 [`crate::anime::index`]。
    pub fn anime_fjall_index_path(&self) -> &'static Path {
        static PATH: OnceLock<PathBuf> = OnceLock::new();
        PATH.get_or_init(|| self.anime_data_path().join(".aki/fjall-index"))
    }

    pub fn bangumi_subject_path(&self) -> &'static Path {
        static PATH: OnceLock<PathBuf> = OnceLock::new();
        PATH.get_or_init(|| self.root.join("bangumi/subject"))
    }

    /// `export-state` 没指定 `--output` 时写出的数据库文件。
    pub fn anime_state_db_path(&self) -> &'static Path {
        static PATH: OnceLock<PathBuf> = OnceLock::new();
        PATH.get_or_init(|| self.root.join("anime-state.db"))
    }

    pub fn game_data_path(&self) -> &'static Path {
        static PATH: OnceLock<PathBuf> = OnceLock::new();
        PATH.get_or_init(|| utils::path::normalize(self.root.join(&self.game.path)))
    }

    /// Steam 原始响应（商店接口与 Web API）的落盘目录。
    pub fn steam_data_path(&self) -> &'static Path {
        static PATH: OnceLock<PathBuf> = OnceLock::new();
        PATH.get_or_init(|| utils::path::normalize(self.root.join("steam")))
    }

    /// 按命名空间 `kind()` 取数据目录，供 `Namespace::data_path()` 的默认实现使用。
    ///
    /// 每个目录单独缓存，并各自归一化路径；未登记的命名空间退回
    /// `{root}/{kind}`。
    pub fn namespace_data_path(&self, kind: &str) -> PathBuf {
        static PATHS: OnceLock<std::sync::Mutex<std::collections::BTreeMap<String, PathBuf>>> =
            OnceLock::new();
        let paths = PATHS.get_or_init(|| std::sync::Mutex::new(std::collections::BTreeMap::new()));
        let mut paths = paths.lock().expect("namespace data path cache poisoned");
        paths
            .entry(kind.to_string())
            .or_insert_with(|| match kind {
                "anime" => self.anime_data_path().to_path_buf(),
                "game" => self.game_data_path().to_path_buf(),
                other => utils::path::normalize(self.root.join(other)),
            })
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_config_with_work_dir() {
        std::env::set_current_dir("database").unwrap();
        let config = Config::load_with_current_dir().unwrap();
        dbg!(&config);
        dbg!(config.anime_data_path());
    }

    #[test]
    #[should_panic]
    fn load_config_with_work_dir_panic() {
        std::env::set_current_dir("alpha").unwrap();
        let _ = Config::load_with_current_dir().unwrap();
    }

    #[test]
    fn lazy_config() {
        dbg!(&*CONFIG);
    }
}
