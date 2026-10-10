use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;

use super::super::{Anime, AnimeInfoInternationalization, SaveAnimeError, index::AnimeIndex};
use crate::source::bangumi::Subject;
use crate::{Language, MetaId};

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
    // 索引只是加速定位的缓存，打不开就退化成遍历数据目录。
    let index = AnimeIndex::open_or_warn();
    // 先确认要更新的条目存在且唯一，避免为一次注定失败的调用写入 Bangumi 数据。
    let (mut anime, outdated_path) = find_exclusive_anime(index.as_ref(), bangumi_id)?;

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
    if let Some(index) = index.as_ref() {
        index.set_anime_index_or_warn(&anime);
        index.flush_or_warn();
    }
    if path != outdated_path {
        std::fs::remove_file(&outdated_path).map_err(UpdateWithBangumiError::RemoveOutdatedFile)?;
    }
    Ok(anime)
}

/// 找出 `bangumi_id` 唯一对应的番剧，并返回它当前的数据文件路径。
///
/// 先查索引；索引不可用、未命中或已失效（文件被改名、删除）时回退到遍历数据
/// 目录，并把找到的映射写回索引。
fn find_exclusive_anime(
    index: Option<&AnimeIndex>,
    bangumi_id: i64,
) -> Result<(Anime, PathBuf), UpdateWithBangumiError> {
    if let Some(anime) = index
        .and_then(|index| index.get_meta_id_of_bangumi_id(bangumi_id))
        .and_then(|meta_id| load_indexed(bangumi_id, &meta_id))
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
    if let Some(index) = index {
        index.set_anime_index_or_warn(&anime);
        index.flush_or_warn();
    }
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
/// - `meta.id`、`info` 与 `bangumi_id` 取自最新条目；
/// - `meta.data.title` 只在最新条目给出了**非空** `name_cn` 时才覆盖（接口对没有中文名
///   的条目的 `name_cn` 是空串，`From<Subject>` 会把它变成 `Some("")`）；
/// - `i18n` 见 [`merge_internationalizations`]，逐语言、逐字段合并；
/// - 由用户填写的 `state` 与记录创建时间 `meta.data.create_time` 保持不变。
fn refresh_with_subject(anime: &mut Anime, subject: Subject) {
    let fresh: Anime = subject.into();
    anime.meta.id = fresh.meta.id;
    if let Some(title) = fresh.meta.data.title.filter(|title| !title.is_empty()) {
        anime.meta.data.title = Some(title);
    }
    anime.info = fresh.info;
    merge_internationalizations(
        &mut anime.internationalizations,
        fresh.internationalizations,
    );
    anime.bangumi_id = fresh.bangumi_id;
}

/// 把最新条目的 `i18n` 合并进已有数据，**逐语言**并**逐字段**（`title` / `summary`）判断。
///
/// `From<Subject>` 给出的每个语言条目只带它那一门语言真正有的东西，另一个字段是 `None`
/// （例如原名是日文、简介是中文时，日文条目只有 `title`，中文条目只有 `summary`），
/// 所以整条替换会把 Bangumi 这次没提到的字段一起抹掉。按字段合并的规则是：
///
/// | 最新条目在这一门语言给的字段 | 处理 |
/// |------------------------------|------|
/// | 非空值 | 覆盖，标题与简介以最新条目为准 |
/// | `None` 或空串 | 保留原值——它可能是用户自己补的翻译，不能因为这次没给就清掉 |
///
/// 因此：用户自备的语言（如英文）不会被碰；两个字段都是空的「空壳语言」不会被新建；
/// 已有的语言条目也不会因为这次没数据而被删掉。
fn merge_internationalizations(
    existing: &mut BTreeMap<Language, AnimeInfoInternationalization>,
    fresh: BTreeMap<Language, AnimeInfoInternationalization>,
) {
    for (language, fresh) in fresh {
        // 接口对「没有这个语言的标题」给的是空串（没有中文名的条目 `name_cn` 就是 `""`），
        // 空串按「没给」处理，否则会把已有的中文标题写成空字符串。
        let title = fresh.title.filter(|title| !title.is_empty());
        let summary = fresh.summary.filter(|summary| !summary.is_empty());
        if title.is_none() && summary.is_none() {
            continue;
        }
        let entry = existing
            .entry(language)
            .or_insert_with(|| AnimeInfoInternationalization::new(None, None));
        if title.is_some() {
            entry.title = title;
        }
        if summary.is_some() {
            entry.summary = summary;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::anime::{AnimeInfo, AnimeInfoInternationalization, AnimeState};
    use crate::{Language, Meta};

    fn subject_example() -> Subject {
        serde_json::from_str(include_str!("../../source/bangumi/subject-example.json")).unwrap()
    }

    /// 拿样本条目改出 `name` / `name_cn` / `summary` 三件套，用来验证语言检测与合并。
    fn subject_with(name: &str, name_cn: &str, summary: &str) -> Subject {
        let mut value: serde_json::Value =
            serde_json::from_str(include_str!("../../source/bangumi/subject-example.json")).unwrap();
        value["name"] = serde_json::Value::String(name.to_string());
        value["name_cn"] = serde_json::Value::String(name_cn.to_string());
        value["summary"] = serde_json::Value::String(summary.to_string());
        serde_json::from_value(value).unwrap()
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
    fn without_an_index_it_falls_back_to_scanning() {
        // 索引不可用（`None`）时，定位必须退化成遍历数据目录，而不是直接失败。
        // 这里用一个几乎不可能存在的 bangumi_id，走的正是遍历后未命中的分支。
        let error = find_exclusive_anime(None, i64::MAX).unwrap_err();
        assert!(
            matches!(error, UpdateWithBangumiError::NotFound(id) if id == i64::MAX),
            "unexpected error: {error}"
        );
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

    fn i18n(anime: &Anime, language: Language) -> (Option<&str>, Option<&str>) {
        anime
            .internationalizations
            .get(&language)
            .map_or((None, None), |entry| {
                (entry.title.as_deref(), entry.summary.as_deref())
            })
    }

    /// 原名是日文、简介也是日文时，最新条目只给中文标题（`name_cn`），
    /// 中文简介是用户自己写的——按字段合并必须更新标题、**留下**简介。
    #[test]
    fn i18n_merge_only_touches_the_fields_bangumi_provides() {
        let subject = subject_with(
            "〈物語〉シリーズ オフ&モンスターシーズン",
            "物语系列 外传季&怪物季",
            "西尾維新のライトノベルを原作とするアニメ。",
        );
        let mut anime = outdated_anime(subject.id);
        anime.internationalizations.insert(
            Language::ZhCn,
            AnimeInfoInternationalization::new(
                Some("旧中文名".to_string()),
                Some("我自己写的简介".to_string()),
            ),
        );

        refresh_with_subject(&mut anime, subject.clone());

        assert_eq!(
            i18n(&anime, Language::ZhCn),
            (Some(subject.name_cn.as_str()), Some("我自己写的简介")),
            "标题来自最新条目，简介不能被 Bangumi 这次的 None 抹掉"
        );
        assert_eq!(
            i18n(&anime, Language::JaJp),
            (
                Some(subject.name.as_str()),
                Some(subject.summary.as_str())
            ),
            "日文条目里 Bangumi 这次给的两个字段都该覆盖"
        );
    }

    /// 没有中文名的条目 `name_cn` 是空串，不能把已有的中文标题写成空。
    #[test]
    fn i18n_merge_treats_empty_strings_as_absent() {
        let subject = subject_with("〈物語〉シリーズ", "", "西尾維新のライトノベル。");
        let mut anime = outdated_anime(subject.id);
        anime.internationalizations.insert(
            Language::ZhCn,
            AnimeInfoInternationalization::new(
                Some("旧中文名".to_string()),
                Some("我自己写的简介".to_string()),
            ),
        );
        let meta_title_before = anime.meta.data.title.clone();

        refresh_with_subject(&mut anime, subject);

        assert_eq!(
            i18n(&anime, Language::ZhCn),
            (Some("旧中文名"), Some("我自己写的简介"))
        );
        // `meta.data.title` 也来自 `name_cn`，空串同样不该覆盖。
        assert_eq!(anime.meta.data.title, meta_title_before);
    }

    /// 空壳语言不新建：Bangumi 只给了日文时，不该凭空多出一个空的中文条目。
    #[test]
    fn i18n_merge_does_not_create_empty_entries() {
        let subject = subject_with("〈物語〉シリーズ", "", "西尾維新のライトノベル。");
        let mut anime = outdated_anime(subject.id);
        anime.internationalizations.clear();

        refresh_with_subject(&mut anime, subject);

        assert!(anime.internationalizations.contains_key(&Language::JaJp));
        assert!(
            !anime.internationalizations.contains_key(&Language::ZhCn),
            "两个字段都是空的语言不该留下条目"
        );
    }
}
