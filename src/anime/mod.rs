pub mod command;
pub mod index;
mod info;
mod internationlization;
mod namespace;
mod score;
mod state;

use crate::source::bangumi::Subject;
use crate::{CONFIG, Language, Meta, MetaId, Namespace};
pub use info::AnimeInfo;
pub use internationlization::AnimeInternationalization;
pub use namespace::AnimeNamespace;
pub use score::AnimeScore;
use serde::{Deserialize, Serialize};
pub use state::{AnimeState, AnimeWatchStatus};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};
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
    pub internationalizations: BTreeMap<Language, AnimeInternationalization>,
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
        internationalizations: BTreeMap<Language, AnimeInternationalization>,
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

    /// 这条番剧的全部标题（原标题 + 各语言翻译标题），**已去重**。
    ///
    /// 索引与搜索都以它为「这条番剧有哪些标题」的唯一来源。去重是必要的：同一部番的
    /// 原名与某个语言的译名常常一模一样（中文原创番、`name_cn` 与原名相同等），不去重
    /// 的话同一个标题会被写两次索引，`build-index` 报出来的标题数也会虚高。
    ///
    /// 保留首次出现的顺序与写法（`BTreeMap` 按 [`Language`] 的声明顺序遍历），空标题
    /// 会被跳过（索引键不能为空，空字符串也不值得拿去匹配）。
    pub fn all_titles(&self) -> impl Iterator<Item = &str> {
        let mut seen = HashSet::new();
        let mut titles = Vec::new();
        for title in std::iter::once(self.info.title.as_str()).chain(
            self.internationalizations
                .values()
                .filter_map(|i| i.title.as_deref()),
        ) {
            if title.trim().is_empty() {
                continue;
            }
            if seen.insert(title) {
                titles.push(title);
            }
        }
        titles.into_iter()
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

    /// 按文件名（也就是 `meta.id` 的字典序，文件名即 `meta.id + ".json"`）排序后遍历。
    ///
    /// [`Self::iter_all`] 的遍历顺序依赖文件系统，重建索引时若直接用它会带着自增 id
    /// 一起走这个不稳定的顺序。这里先只收集文件路径并排序，再逐条读取，让插入顺序与
    /// `meta.id` 一致，同时不把全部 [`Anime`] 一次性读进内存。
    pub fn iter_all_sorted() -> impl Iterator<Item = Anime> {
        let anime_dir = AnimeNamespace::data_path();
        let mut paths: Vec<PathBuf> = std::fs::read_dir(anime_dir)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                (path.extension()?.to_str()? == "json").then_some(path)
            })
            .collect();
        paths.sort();
        paths
            .into_iter()
            .filter_map(|path| match Self::load(&path) {
                Ok(anime) => Some(anime),
                Err(e) => {
                    eprintln!("[?] failed to load anime file '{}': {}", path.display(), e);
                    None
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
            if let Some(AnimeInternationalization {
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
            subject.name
        );
        let mut i18n = BTreeMap::new();
        let name_language = Language::detect_from(&subject.name);
        let summary_language = Language::detect_from(&subject.summary);
        if name_language == summary_language {
            i18n.insert(
                name_language,
                AnimeInternationalization::new(
                    Some(subject.name.clone()),
                    Some(subject.summary.clone()),
                ),
            );
        } else {
            i18n.insert(
                name_language,
                AnimeInternationalization::new(Some(subject.name.clone()), None),
            );
            i18n.insert(
                summary_language,
                AnimeInternationalization::new(None, Some(subject.summary.clone())),
            );
        }
        // 由于name_cn的特殊性，故此处单独处理zh-cn
        i18n.entry(Language::ZhCn)
            .and_modify(|v| v.title = Some(subject.name_cn.clone()))
            .or_insert(AnimeInternationalization::new(
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
                // Bangumi 的 tags 已按使用人数降序，直接沿用这个顺序
                tags: subject.tags.iter().map(|tag| tag.name.clone()).collect(),
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

    /// 同名标题（原名 == 某语言译名）、空标题、跨语言重复都只算一个。
    #[test]
    fn all_titles_is_deduplicated() {
        let anime = Anime::new(
            Meta::with_id("[2020-01-01] 测试".to_string(), None),
            AnimeInfo {
                title: "原名".to_string(),
                summary: None,
                platform: None,
                air_date: None,
                season: None,
                episodes: None,
                tags: Vec::new(),
            },
            BTreeMap::from([
                // 与原名重复（中文原创番就是这样：`name` == `name_cn`）
                (
                    Language::ZhCn,
                    AnimeInternationalization::new(Some("原名".to_string()), None),
                ),
                // 空标题：跳过
                (
                    Language::JaJp,
                    AnimeInternationalization::new(Some("   ".to_string()), None),
                ),
                // 真正不同的译名
                (
                    Language::EnUs,
                    AnimeInternationalization::new(Some("Original Title".to_string()), None),
                ),
            ]),
            AnimeState::default(),
            None,
        );

        assert_eq!(
            anime.all_titles().collect::<Vec<_>>(),
            vec!["原名", "Original Title"]
        );
    }

    #[test]
    fn test_from_bangumi() {
        let subject_json_str = include_str!("../source/bangumi/subject-example.json");
        let subject: Subject = serde_json::from_str(subject_json_str).unwrap();
        let anime = Anime::from(subject);
        dbg!(anime);
    }

    /// 条目里的标签按接口给的顺序（使用人数降序）进 `info.tags`，只留名字。
    #[test]
    fn tags_come_from_the_subject() {
        let subject: Subject =
            serde_json::from_str(include_str!("../source/bangumi/subject-example.json")).unwrap();
        let expected: Vec<String> = subject.tags.iter().map(|tag| tag.name.clone()).collect();
        assert!(!expected.is_empty(), "样本条目应当带标签");

        let anime = Anime::from(subject);
        assert_eq!(anime.info.tags, expected);
        assert_eq!(
            anime.info.tags.first().map(String::as_str),
            Some("物语系列")
        );
    }

    /// 老数据文件里没有 `tags` 字段，读的时候应当当成空列表而不是报错。
    #[test]
    fn tags_default_to_empty_for_old_files() {
        let subject: Subject =
            serde_json::from_str(include_str!("../source/bangumi/subject-example.json")).unwrap();
        let mut value = serde_json::to_value(Anime::from(subject)).unwrap();
        // 早期版本写出来的文件里没有 info.tags
        value["info"].as_object_mut().unwrap().remove("tags");

        let old: Anime = serde_json::from_value(value).unwrap();
        assert!(old.info.tags.is_empty());
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
