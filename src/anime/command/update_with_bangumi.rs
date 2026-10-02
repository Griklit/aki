use std::io;
use std::path::PathBuf;

use super::super::{Anime, SaveAnimeError, index::BangumiIdIndex};
use crate::MetaId;
use crate::bangumi::Subject;

#[derive(Debug, thiserror::Error)]
pub enum UpdateWithBangumiError {
    #[error("bangumi request error")]
    BangumiRequest(#[source] reqwest::Error),
    #[error("save bangumi subject error")]
    SaveBangumiSubject(#[source] io::Error),
    #[error("no anime found with bangumi id {0}")]
    NotFound(i64),
    #[error("{1} anime entries share bangumi id {0}")]
    Ambiguous(i64, usize),
    #[error("save anime error")]
    SaveAnime(#[source] SaveAnimeError),
    #[error("anime file already exists: {}", .0.display())]
    TargetExists(PathBuf),
    #[error("remove outdated anime file error")]
    RemoveOutdatedFile(#[source] io::Error),
}

pub async fn update_with_bangumi(
    client: &reqwest::Client,
    bangumi_id: i64,
) -> Result<Anime, UpdateWithBangumiError> {
    let mut index = BangumiIdIndex::load();
    // 先确认要更新的条目存在且唯一，避免为一次注定失败的调用写入 Bangumi 数据。
    let (mut anime, outdated_path) = find_exclusive_anime(&mut index, bangumi_id)?;

    let subject = Subject::request_with_id(client, bangumi_id)
        .await
        .map_err(UpdateWithBangumiError::BangumiRequest)?;
    subject
        .save()
        .map_err(UpdateWithBangumiError::SaveBangumiSubject)?;

    refresh_with_subject(&mut anime, subject);
    let path = anime.meta.id.json_data_path();

    // 不覆盖其他条目：同日同名的番剧会算出同一个 `meta.id`。
    if path != outdated_path && path.exists() {
        return Err(UpdateWithBangumiError::TargetExists(path));
    }
    anime
        .save_as_meta_id_force()
        .map_err(UpdateWithBangumiError::SaveAnime)?;
    // `meta.id` 可能已按最新数据重算：先把索引指向新文件，再删除旧文件。
    index.set(bangumi_id, &anime.meta.id.id);
    if path != outdated_path {
        std::fs::remove_file(&outdated_path).map_err(UpdateWithBangumiError::RemoveOutdatedFile)?;
    }
    Ok(anime)
}

/// 找出 `bangumi_id` 唯一对应的番剧，并返回它当前的数据文件路径。
///
/// 先查 `.aki` 下的 bangumi-id 索引；索引缺失或已失效（文件被改名、删除）时
/// 回退到遍历数据目录，并把找到的映射写回索引。
fn find_exclusive_anime(
    index: &mut BangumiIdIndex,
    bangumi_id: i64,
) -> Result<(Anime, PathBuf), UpdateWithBangumiError> {
    if let Some(anime) = index
        .get(bangumi_id)
        .and_then(|meta_id| load_indexed(bangumi_id, meta_id))
    {
        let path = anime.meta.id.json_data_path();
        return Ok((anime, path));
    }

    let mut animes = Anime::iter_all().filter(|anime| anime.bangumi_id == Some(bangumi_id));
    let Some(anime) = animes.next() else {
        return Err(UpdateWithBangumiError::NotFound(bangumi_id));
    };
    let duplicated = animes.count();
    if duplicated > 0 {
        return Err(UpdateWithBangumiError::Ambiguous(
            bangumi_id,
            duplicated + 1,
        ));
    }
    index.set(bangumi_id, &anime.meta.id.id);
    let path = anime.meta.id.json_data_path();
    Ok((anime, path))
}

/// 按索引里的 `meta_id` 读取番剧，并确认它确实属于该 `bangumi_id`。
fn load_indexed(bangumi_id: i64, meta_id: &str) -> Option<Anime> {
    let anime = Anime::load_with_meta_id(&MetaId::with_id(meta_id.to_string())).ok()?;
    (anime.bangumi_id == Some(bangumi_id)).then_some(anime)
}

/// 用 Bangumi 的最新条目刷新 `anime` 中由 Bangumi 提供的部分。
///
/// - `meta.id`、`meta.data.title`、`info` 与 `bangumi_id` 取自最新条目；
/// - `i18n` 按语言合并：Bangumi 能提供的语言整体覆盖，其余语言保留；
/// - 由用户填写的 `state` 与记录创建时间 `meta.data.create_time` 保持不变。
fn refresh_with_subject(anime: &mut Anime, subject: Subject) {
    let fresh: Anime = subject.into();
    anime.meta.id = fresh.meta.id;
    anime.meta.data.title = fresh.meta.data.title;
    anime.info = fresh.info;
    anime
        .internationalizations
        .extend(fresh.internationalizations);
    anime.bangumi_id = fresh.bangumi_id;
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::{AnimeInfo, AnimeInfoInternationalization, AnimeState, Language, Meta};

    fn subject_example() -> Subject {
        serde_json::from_str(include_str!("../../bangumi/subject-example.json")).unwrap()
    }

    fn outdated_anime(bangumi_id: i64) -> Anime {
        let mut state = AnimeState::default();
        state.watching();
        Anime::new(
            Meta::with_id(
                "[1999-01-01] 旧标题".to_string(),
                Some("旧标题".to_string()),
            ),
            AnimeInfo {
                title: "旧标题".to_string(),
                summary: Some("旧简介".to_string()),
                platform: Some("TV".to_string()),
                air_date: None,
                season: None,
                episodes: Some(1),
            },
            BTreeMap::from([(
                Language::EnUs,
                AnimeInfoInternationalization::new(Some("Old Title".to_string()), None),
            )]),
            state,
            Some(bangumi_id),
        )
    }

    #[test]
    fn refresh_replaces_bangumi_data_and_keeps_user_data() {
        let subject = subject_example();
        let mut anime = outdated_anime(subject.id);
        let state_before = serde_json::to_value(&anime.state).unwrap();
        let create_time_before = anime.meta.data.create_time;

        refresh_with_subject(&mut anime, subject.clone());

        // 用户填写的 state 与记录创建时间原样保留。
        assert_eq!(serde_json::to_value(&anime.state).unwrap(), state_before);
        assert_eq!(anime.meta.data.create_time, create_time_before);
        // Bangumi 提供的部分换成最新数据。
        assert_eq!(anime.info.title, subject.name);
        assert_eq!(
            anime.info.summary.as_deref(),
            Some(subject.summary.as_str())
        );
        assert_eq!(
            anime.info.platform.as_deref(),
            Some(subject.platform.as_str())
        );
        assert_eq!(
            anime.meta.data.title.as_deref(),
            Some(subject.name_cn.as_str())
        );
        assert_eq!(anime.bangumi_id, Some(subject.id));
        // meta.id 按最新数据重算，不再沿用旧标题。
        assert_eq!(
            anime.meta.id.id,
            format!("[{}] {}", subject.date.unwrap(), subject.name)
        );
        // i18n 合并：Bangumi 提供的语言写入，用户自备的语言保留。
        assert!(anime.internationalizations.contains_key(&Language::JaJp));
        assert!(anime.internationalizations.contains_key(&Language::ZhCn));
        assert!(anime.internationalizations.contains_key(&Language::EnUs));
        assert_eq!(
            anime
                .internationalizations
                .get(&Language::EnUs)
                .and_then(|i| i.title.as_deref()),
            Some("Old Title")
        );
    }
}
