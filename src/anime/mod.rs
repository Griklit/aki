pub mod command;
pub mod index;
mod info;
mod namespace;
mod score;
mod state;

use crate::source::bangumi::Subject;
use crate::{CONFIG, Language, Meta, MetaId, Namespace};
pub use info::{AnimeInfo, AnimeInfoInternationalization};
pub use namespace::AnimeNamespace;
pub use score::AnimeScore;
use serde::{Deserialize, Serialize};
pub use state::{AnimeState, AnimeWatchStatus};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Anime {
    pub meta: Meta<AnimeNamespace>,
    pub info: AnimeInfo,
    #[serde(rename = "i18n")]
    pub internationalizations: BTreeMap<Language, AnimeInfoInternationalization>,
    pub state: AnimeState,
    pub bangumi_id: Option<i64>,
}

#[derive(Debug, thiserror::Error)]
pub enum LoadAnimeError {
    #[error("io error")]
    Io(#[source] io::Error),
    #[error("json deserialize error")]
    JsonDeserialize(#[source] serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum SaveAnimeError {
    #[error("io error")]
    Io(#[source] io::Error),
    #[error("json serialize error")]
    JsonSerialize(#[source] serde_json::Error),
    #[error("file exists")]
    Exists,
}

impl Anime {
    #[must_use]
    pub fn new(
        meta: Meta<AnimeNamespace>,
        info: AnimeInfo,
        internationalizations: BTreeMap<Language, AnimeInfoInternationalization>,
        state: AnimeState,
        bangumi_id: Option<i64>,
    ) -> Self {
        Anime {
            meta,
            info,
            internationalizations,
            state,
            bangumi_id,
        }
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

    /// 这条番剧的全部标题（原标题 + 各语言翻译标题）。
    ///
    /// 索引与搜索都以它为「这条番剧有哪些标题」的唯一来源；空标题会被跳过
    /// （索引键不能为空，空字符串也不值得拿去匹配）。
    pub fn all_titles(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.info.title.as_str())
            .chain(
                self.internationalizations
                    .values()
                    .filter_map(|i| i.title.as_deref()),
            )
            .filter(|title| !title.trim().is_empty())
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, LoadAnimeError> {
        let file = File::open(path).map_err(LoadAnimeError::Io)?;
        let anime: Anime =
            serde_json::from_reader(file).map_err(LoadAnimeError::JsonDeserialize)?;
        Ok(anime)
    }

    pub fn load_with_meta_id(meta_id: &MetaId<AnimeNamespace>) -> Result<Self, LoadAnimeError> {
        let path = meta_id.json_data_path();
        Self::load(path)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), SaveAnimeError> {
        let path = path.as_ref();
        if path.exists() {
            return Err(SaveAnimeError::Exists);
        }
        let file = File::create(path).map_err(SaveAnimeError::Io)?;
        serde_json::to_writer_pretty(file, self).map_err(SaveAnimeError::JsonSerialize)?;
        Ok(())
    }

    pub fn save_force(&self, path: impl AsRef<Path>) -> Result<(), SaveAnimeError> {
        let path = path.as_ref();
        let file = File::create(path).map_err(SaveAnimeError::Io)?;
        serde_json::to_writer_pretty(file, self).map_err(SaveAnimeError::JsonSerialize)?;
        Ok(())
    }

    pub fn save_as_meta_id(&self) -> Result<(), SaveAnimeError> {
        let path = self.meta.id.json_data_path();
        self.save(path)?;
        Ok(())
    }

    pub fn save_as_meta_id_force(&self) -> Result<(), SaveAnimeError> {
        let path = self.meta.id.json_data_path();
        self.save_force(path)?;
        Ok(())
    }

    pub fn iter_all() -> impl Iterator<Item = Anime> {
        Self::iter_all_with_path().map(|(_, anime)| anime)
    }

    /// 与 [`Self::iter_all`] 相同，但同时给出每条数据是从哪个文件读出来的。
    ///
    /// 需要原地改写这条数据的命令用它，才能区分「文件就在规范路径上」和
    /// 「文件名与 `meta.id` 不一致」这两种情况。
    pub fn iter_all_with_path() -> impl Iterator<Item = (PathBuf, Anime)> {
        let anime_dir = AnimeNamespace::data_path();
        std::fs::read_dir(anime_dir)
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
                    Ok(anime) => Some((path, anime)),
                    Err(e) => {
                        eprintln!("[?] failed to load anime file '{}': {}", path.display(), e);
                        None
                    }
                }
            })
    }

    #[must_use]
    pub fn bangumi_subject(&self) -> Option<Result<Subject, io::Error>> {
        Some(Subject::load_with_id(self.bangumi_id?))
    }
}

impl fmt::Display for Anime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let language = Language::from_str(&CONFIG.locale);
        write!(
            f,
            "[{}] ",
            self.info.air_date.map_or_else(
                || "unknown".to_string(),
                |d| d.format("%Y-%m-%d").to_string()
            )
        )?;
        let title: Cow<str> = if let Ok(language) = language {
            let trans = self.internationalizations.get(&language);
            if let Some(AnimeInfoInternationalization {
                title: Some(trans_title),
                ..
            }) = trans
                && trans_title != &self.info.title
            {
                format!("{} ({})", &trans_title, &self.info.title).into()
            } else {
                Cow::from(&self.info.title)
            }
        } else {
            Cow::from(&self.info.title)
        };
        write!(f, "{title}")
    }
}

impl From<Subject> for Anime {
    fn from(subject: Subject) -> Self {
        let meta_id = format!(
            "[{}] {}",
            subject
                .date
                .map_or("unknown".to_string(), |d| d.format("%Y-%m-%d").to_string()),
            &subject.name
        );
        let mut i18n = BTreeMap::new();
        let name_language = Language::detect_from(&subject.name);
        let summary_language = Language::detect_from(&subject.summary);
        if name_language == summary_language {
            i18n.insert(
                name_language,
                AnimeInfoInternationalization::new(
                    Some(subject.name.clone()),
                    Some(subject.summary.clone()),
                ),
            );
        } else {
            i18n.insert(
                name_language,
                AnimeInfoInternationalization::new(Some(subject.name.clone()), None),
            );
            i18n.insert(
                summary_language,
                AnimeInfoInternationalization::new(None, Some(subject.summary.clone())),
            );
        }
        // 由于name_cn的特殊性，故此处单独处理zh-cn
        i18n.entry(Language::ZhCn)
            .and_modify(|v| v.title = Some(subject.name_cn.clone()))
            .or_insert(AnimeInfoInternationalization::new(
                Some(subject.name_cn.clone()),
                None,
            ));
        Self {
            meta: Meta::with_id(meta_id, Some(subject.name_cn)),
            info: AnimeInfo {
                title: subject.name,
                summary: Some(subject.summary), // 无法确定anime一定不是国漫故而此处summary同样不能确定是原文。
                platform: Some(subject.platform),
                air_date: subject.date,
                season: None,
                episodes: Some(u16::try_from(subject.eps).unwrap_or_default()),
            },
            internationalizations: i18n,
            state: AnimeState::default(),
            bangumi_id: Some(subject.id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_bangumi() {
        let subject_json_str = include_str!("../source/bangumi/subject-example.json");
        let subject: Subject = serde_json::from_str(subject_json_str).unwrap();
        let anime = Anime::from(subject);
        dbg!(anime);
    }

    #[test]
    fn load_anime() {
        let anime =
            Anime::load_with_meta_id(&MetaId::with_id("[2010-10-04] 侵略!イカ娘".to_string()))
                .unwrap();
        dbg!(anime);
    }

    #[test]
    #[ignore = "cannot panic: only prints every entry from the local database"]
    fn iter_all_anime() {
        for anime in Anime::iter_all() {
            dbg!(anime.meta.data.title);
        }
    }
}
