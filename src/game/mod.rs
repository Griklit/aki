//! 游戏领域模型，与 `anime` 模块同构。
//!
//! 与番剧的主要差别：发售日期粒度不一（见 [`GameRelease`]）、游玩时长以秒记录、
//! 来源 id 分开存放（`steam_app_id` / `bangumi_id`）。

pub mod command;
mod info;
mod namespace;
// mod score; 评分模型还没定，先整个注释掉
pub mod state;
pub mod steam;

use crate::{CONFIG, Language, Meta, MetaId, Namespace};
pub use info::{ExternalScore, GameInfo, GameInfoInternationalization, GamePlatform, GameRelease};
pub use namespace::GameNamespace;
use serde::{Deserialize, Serialize};
pub use state::{GameAchievements, GamePlayStatus, GameState};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, thiserror::Error)]
pub enum LoadGameError {
    #[error("io error")]
    Io(#[source] io::Error),
    #[error("json deserialize error")]
    JsonDeserialize(#[source] serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum SaveGameError {
    #[error("io error")]
    Io(#[source] io::Error),
    #[error("json serialize error")]
    JsonSerialize(#[source] serde_json::Error),
    #[error("file exists")]
    Exists,
}

/// 游戏聚合根，与 [`Anime`](crate::Anime) 同构。
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Game {
    pub meta: Meta<GameNamespace>,
    pub info: GameInfo,
    #[serde(rename = "i18n")]
    pub internationalizations: BTreeMap<Language, GameInfoInternationalization>,
    pub state: GameState,
    pub steam_app_id: Option<u32>,
    pub bangumi_id: Option<i64>,
}

impl Game {
    #[must_use]
    pub fn new(
        meta: Meta<GameNamespace>,
        info: GameInfo,
        internationalizations: BTreeMap<Language, GameInfoInternationalization>,
        state: GameState,
        steam_app_id: Option<u32>,
        bangumi_id: Option<i64>,
    ) -> Self {
        Self {
            meta,
            info,
            internationalizations,
            state,
            steam_app_id,
            bangumi_id,
        }
    }

    /// 更换用户手填的状态（整体替换）。
    pub fn set_state(&mut self, state: GameState) {
        self.state = state;
    }

    #[must_use]
    pub fn internationalization_titles(&self) -> Vec<(Language, &str)> {
        self.internationalizations
            .iter()
            .filter_map(|(l, i)| i.title.as_deref().map(|v| (*l, v)))
            .collect()
    }

    #[must_use]
    pub fn internationalization_summaries(&self) -> Vec<(Language, &str)> {
        self.internationalizations
            .iter()
            .filter_map(|(l, i)| i.summary.as_deref().map(|v| (*l, v)))
            .collect()
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, LoadGameError> {
        let file = File::open(path).map_err(LoadGameError::Io)?;
        let game: Self = serde_json::from_reader(file).map_err(LoadGameError::JsonDeserialize)?;
        Ok(game)
    }

    pub fn load_with_meta_id(meta_id: &MetaId<GameNamespace>) -> Result<Self, LoadGameError> {
        Self::load(meta_id.json_data_path())
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), SaveGameError> {
        let path = path.as_ref();
        if path.exists() {
            return Err(SaveGameError::Exists);
        }
        self.write(path)
    }

    /// 与 [`Self::save`] 相同，但允许覆盖已有文件。
    pub fn save_force(&self, path: impl AsRef<Path>) -> Result<(), SaveGameError> {
        self.write(path.as_ref())
    }

    fn write(&self, path: &Path) -> Result<(), SaveGameError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(SaveGameError::Io)?;
        }
        let file = File::create(path).map_err(SaveGameError::Io)?;
        serde_json::to_writer_pretty(file, self).map_err(SaveGameError::JsonSerialize)?;
        Ok(())
    }

    pub fn save_as_meta_id(&self) -> Result<(), SaveGameError> {
        self.save(self.meta.id.json_data_path())
    }

    pub fn save_as_meta_id_force(&self) -> Result<(), SaveGameError> {
        self.save_force(self.meta.id.json_data_path())
    }

    pub fn iter_all() -> impl Iterator<Item = Self> {
        Self::iter_all_with_path().map(|(_, game)| game)
    }

    /// 与 [`Self::iter_all`] 相同，但同时给出每条数据是从哪个文件读出来的。
    pub fn iter_all_with_path() -> impl Iterator<Item = (PathBuf, Self)> {
        let game_dir = GameNamespace::data_path();
        std::fs::read_dir(game_dir)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.extension()?.to_str()? != "json" {
                    return None;
                }
                match Self::load(&path) {
                    Ok(game) => Some((path, game)),
                    Err(e) => {
                        eprintln!("[?] failed to load game file '{}': {}", path.display(), e);
                        None
                    }
                }
            })
    }
}

impl fmt::Display for Game {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] ", self.info.release_prefix())?;
        let title: Cow<str> = if let Ok(language) = Language::from_str(&CONFIG.locale) {
            let trans = self.internationalizations.get(&language);
            if let Some(GameInfoInternationalization {
                title: Some(trans_title),
                ..
            }) = trans
                && trans_title != &self.info.title
            {
                format!("{} ({})", trans_title, self.info.title).into()
            } else {
                Cow::from(&self.info.title)
            }
        } else {
            Cow::from(&self.info.title)
        };
        write!(f, "{title}")?;
        if let Some(seconds) = self.state.played_seconds() {
            write!(f, " — {seconds} 秒")?;
        }
        if let Some(achievements) = self.state.achievements() {
            write!(f, " — 成就 {achievements}")?;
        }
        Ok(())
    }
}
