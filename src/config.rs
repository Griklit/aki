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

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigAnime {
    pub path: PathBuf,
    #[serde(default = "default_bangimi_api")]
    pub bangumi_api: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default, skip)]
    pub root: PathBuf,
    #[serde(default, skip)]
    pub locale: String,
    pub anime: ConfigAnime,
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

    pub fn anime_tantivy_index_path(&self) -> &'static Path {
        static PATH: OnceLock<PathBuf> = OnceLock::new();
        PATH.get_or_init(|| self.anime_data_path().join(".aki/tantivy-index"))
    }

    pub fn anime_fjall_index_path(&self) -> &'static Path {
        static PATH: OnceLock<PathBuf> = OnceLock::new();
        PATH.get_or_init(|| self.anime_data_path().join(".aki/fjall-index"))
    }

    pub fn anime_bangumi_id_index_path(&self) -> &'static Path {
        static PATH: OnceLock<PathBuf> = OnceLock::new();
        PATH.get_or_init(|| self.anime_data_path().join(".aki/bangumi-id-index.json"))
    }

    pub fn bangumi_subject_path(&self) -> &'static Path {
        static PATH: OnceLock<PathBuf> = OnceLock::new();
        PATH.get_or_init(|| self.root.join("bangumi/subject"))
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
