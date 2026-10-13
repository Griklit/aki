use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::super::{Anime, AnimeNamespace, MetaId, index};
use crate::Namespace;

/// 按标题匹配字符串（可用标签筛选），返回分数最高的前 `limit` 条。
///
/// 标题走 `.aki/anime-index.sqlite` 的 `titles` 表：把整张表扫一遍做字符串匹配，
/// 只对最终要返回的那几条去读数据文件。`tags` 非空时再用 `tags`/`anime_tags` 里的
/// 标签集合过滤，多个标签之间是「且」。
///
/// `query` 为空、只给 `tags` 时，就是「按标签列番剧」：按 `meta.id` 顺序（也就是
/// `[日期] 名字` 的日期顺序）返回，不走标题打分。
///
/// 索引不存在、打不开、或者里面还没有标题/标签（没跑过对应版本的 `build-index`）时
/// 退回扫数据目录，这样 `search` 在没有索引时依然可用，只是慢一些——这几种情况会打印
/// 一行 `[?]` 说明为什么走了文件分支（打不开时 `is_available` 已经报过原因，不再重复）。
///
/// 索引可用但**一条都没匹配到**时，默认就到此为止（索引是权威的）。`verify_index`
/// 为 `true` 时再扫一遍数据文件确认：索引可能落后于数据目录（手工放过文件、或加了
/// 条目没重建索引），真扫到了就提示索引过期，并把这批文件结果返回。确认扫描要遍历
/// 整个数据目录，所以只在显式要求时做（`aki-cli anime search --verify-index`）。
#[must_use]
pub async fn search(query: &str, tags: &[String], limit: usize, verify_index: bool) -> Vec<Anime> {
    let query = query.trim();
    let tags = normalize_tags(tags);
    match (query.is_empty(), tags.is_empty()) {
        // 什么都没有：不返回整个库。
        (true, true) => Vec::new(),
        // 只按标签找。
        (true, false) => search_by_tags(&tags, limit, verify_index).await,
        // 按标题找（标签可选，命中后再过滤）。
        (false, _) => search_by_title(query, &tags, limit, verify_index).await,
    }
}

/// 去掉空白、统一小写：标签在索引里就是按这个形式存的。
fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut normalized: Vec<String> = tags
        .iter()
        .map(|tag| tag.trim().to_lowercase())
        .filter(|tag| !tag.is_empty())
        .collect();
    normalized.sort();
    normalized.dedup();
    normalized
}

/// 标题搜索：先拿到「`meta.id` → 分数」，再按标签过滤，最后排序截断。
///
/// 过滤放在截断**之前**，所以 `--tag` 不会让结果少于一页应有的条数。
async fn search_by_title(
    query: &str,
    tags: &[String],
    limit: usize,
    verify_index: bool,
) -> Vec<Anime> {
    match lookup_index(query).await {
        // 索引里有候选：按标签过滤后排序截断。
        IndexLookup::Hits(best) => {
            let hits = rank(filter_by_tags(best, tags).await, limit);
            if !hits.is_empty() || !verify_index {
                return hits;
            }
            // 索引里一条都没有，但被要求确认：扫一遍数据文件看看。
            let scanned = rank(filter_by_tags(file_scores(query), tags).await, limit);
            let index_path = AnimeNamespace::sqlite_index_path();
            if let Some(hint) = table_hint("title", State::Missed(true), scanned.len(), &index_path)
            {
                eprintln!("[?] {hint}");
            }
            scanned
        }
        // 索引不可用（目录不存在 / 打不开 / 表是空的）：扫数据文件，并说明原因。
        lookup @ (IndexLookup::Missing | IndexLookup::Empty | IndexLookup::Unreadable) => {
            let scanned = rank(filter_by_tags(file_scores(query), tags).await, limit);
            let index_path = AnimeNamespace::sqlite_index_path();
            if let Some(hint) = fallback_hint(&lookup, scanned.len(), &index_path) {
                eprintln!("[?] {hint}");
            }
            scanned
        }
    }
}

async fn filter_by_tags(best: BTreeMap<String, f32>, tags: &[String]) -> BTreeMap<String, f32> {
    if tags.is_empty() {
        return best;
    }
    let hits = match lookup_tags(tags).await {
        TagLookup::Hits(hits) => hits,
        _ => read_records()
            .into_iter()
            .filter(|record| record.has_tags(tags))
            .map(|record| record.meta.id.id)
            .collect(),
    };
    best.into_iter()
        .filter(|(meta_id, _)| hits.contains(meta_id))
        .collect()
}
/// 按标签找番剧（多个标签之间是「且」）。
///
/// 结果按 `meta.id` 排序（`[YYYY-MM-DD] 名字` 正好是日期顺序）。
async fn search_by_tags(tags: &[String], limit: usize, verify_index: bool) -> Vec<Anime> {
    let lookup = lookup_tags(tags).await;
    match &lookup {
        TagLookup::Hits(hits) if !hits.is_empty() => return load_meta_ids(hits.clone(), limit),
        // 索引里没有，而又没要求确认：索引说了没有就是没有。
        TagLookup::Hits(_) if !verify_index => return Vec::new(),
        _ => {}
    }
    let scanned = scan_tag_files(tags, limit);
    let index_path = AnimeNamespace::sqlite_index_path();
    if let Some(hint) = tag_fallback_hint(&lookup, scanned.len(), &index_path) {
        eprintln!("[?] {hint}");
    }
    scanned
}

/// 查一次索引的结果。
enum IndexLookup {
    /// 索引可用，返回命中的「`meta.id` → 标题分数」（可能是空的——索引里确实没有）。
    Hits(BTreeMap<String, f32>),
    /// 索引目录还不存在（没建过索引）。
    Missing,
    /// 索引存在，但对应的表里一条数据都没有。
    Empty,
    /// 索引打不开（[`index::is_available`] 已经打印过原因）。
    Unreadable,
}

/// 在索引的标题表上按标题匹配（匹配在 SQL 内完成，打分在 Rust 里做）；
/// 表里一条标题都没有时返回 [`IndexLookup::Empty`]。
async fn lookup_index(query: &str) -> IndexLookup {
    if !AnimeNamespace::sqlite_index_path().exists() {
        return IndexLookup::Missing;
    }
    if !index::is_available() {
        return IndexLookup::Unreadable;
    }
    if !index::has_title_entries().await {
        return IndexLookup::Empty;
    }
    // 同一个 `meta.id` 有多个标题时取最高分。
    let mut best: BTreeMap<String, f32> = BTreeMap::new();
    for (meta_id, title) in index::match_meta_ids_of_title(query).await {
        let Some(score) = score_title(&title, query) else {
            continue;
        };
        match best.get(&meta_id) {
            Some(&existing) if existing >= score => {}
            _ => {
                best.insert(meta_id, score);
            }
        }
    }
    IndexLookup::Hits(best)
}

/// 查索引的标签表的结果，形状与 [`IndexLookup`] 一致。
enum TagLookup {
    /// 索引可用，给出命中的 `meta.id` 集合（可能是空的——索引里确实没有这个标签）。
    Hits(BTreeSet<String>),
    /// 索引目录还不存在（没建过索引）。
    Missing,
    /// 索引存在，但标签表里一条标签都没有（这份索引是加标签之前建的）。
    Empty,
    /// 索引打不开（[`index::is_available`] 已经打印过原因）。
    Unreadable,
}

/// 查索引的标签表，返回命中的 `meta.id` 集合（多个标签取交集）。
async fn lookup_tags(tags: &[String]) -> TagLookup {
    if !AnimeNamespace::sqlite_index_path().exists() {
        return TagLookup::Missing;
    }
    if !index::is_available() {
        return TagLookup::Unreadable;
    }
    if !index::has_tag_entries().await {
        return TagLookup::Empty;
    }
    let mut hits: Option<BTreeSet<String>> = None;
    for tag in tags {
        let meta_ids: BTreeSet<String> =
            index::get_meta_ids_of_tag(tag).await.into_iter().collect();
        hits = Some(match hits {
            None => meta_ids,
            Some(acc) => acc.intersection(&meta_ids).cloned().collect(),
        });
        if hits.as_ref().is_some_and(BTreeSet::is_empty) {
            break;
        }
    }
    TagLookup::Hits(hits.unwrap_or_default())
}

/// 走文件分支时该给的提示（正文，打印由调用方负责）。
///
/// 只在「用户以为在查索引、其实没查到」的时候提示；索引真的没有这条数据、
/// 或者索引打不开（原因已经打印过）时都不提示，免得刷屏。
fn fallback_hint(lookup: &IndexLookup, scanned: usize, index_path: &Path) -> Option<String> {
    table_hint("title", lookup_state(lookup), scanned, index_path)
}

/// 标签查询走文件分支时的提示，规则与 [`fallback_hint`] 相同。
fn tag_fallback_hint(lookup: &TagLookup, scanned: usize, index_path: &Path) -> Option<String> {
    table_hint("tag", tag_state(lookup), scanned, index_path)
}

/// 提示里要区分的几种情况。
#[derive(Clone, Copy)]
enum State {
    /// 索引目录还不存在。
    Missing,
    /// 表存在，但整张表是空的。
    Empty,
    /// 表可用，但这次一条都没匹配到（`true`）。
    Missed(bool),
}

fn lookup_state(lookup: &IndexLookup) -> State {
    match lookup {
        IndexLookup::Missing => State::Missing,
        IndexLookup::Empty => State::Empty,
        IndexLookup::Hits(hits) => State::Missed(hits.is_empty()),
        IndexLookup::Unreadable => State::Missed(false),
    }
}

fn tag_state(lookup: &TagLookup) -> State {
    match lookup {
        TagLookup::Missing => State::Missing,
        TagLookup::Empty => State::Empty,
        TagLookup::Hits(hits) => State::Missed(hits.is_empty()),
        TagLookup::Unreadable => State::Missed(false),
    }
}

fn table_hint(table: &str, state: State, scanned: usize, index_path: &Path) -> Option<String> {
    match state {
        State::Missing => Some(format!(
            "anime index '{}' not found, searching data files instead \
             (run 'aki-cli anime build-index' to build it)",
            index_path.display()
        )),
        State::Empty => Some(format!(
            "anime index '{}' has no {table} entry, searching data files instead \
             (run 'aki-cli anime build-index' to rebuild it)",
            index_path.display()
        )),
        // 索引没匹配到、而文件扫到了：说明索引落后于数据目录。
        State::Missed(true) if scanned > 0 => Some(format!(
            "no hit for that {table} in anime index '{}' but scanning data files \
             found {scanned} anime, the index may be out of date \
             (run 'aki-cli anime build-index' to rebuild it)",
            index_path.display()
        )),
        State::Missed(_) => None,
    }
}

/// 遍历数据目录给标题打分（没有可用索引时的退路）。
///
/// 只读每个文件里的标题、标签与 `meta.id`，不解析整个 [`Anime`]，因此不需要为了搜一次
/// 标题把 state / i18n 简介 / infobox 这些大块数据全部反序列化。
fn file_scores(query: &str) -> BTreeMap<String, f32> {
    let query = query.to_lowercase();
    let mut best: BTreeMap<String, f32> = BTreeMap::new();
    for record in read_records() {
        let Some(score) = record
            .titles()
            .filter_map(|title| score_title(title, &query))
            .reduce(f32::max)
        else {
            continue;
        };
        match best.get(&record.meta.id.id) {
            Some(&existing) if existing >= score => {}
            _ => {
                best.insert(record.meta.id.id, score);
            }
        }
    }
    best
}

/// 扫数据文件找带这些标签的条目，按 `meta.id` 顺序取前 `limit` 条。
fn scan_tag_files(tags: &[String], limit: usize) -> Vec<Anime> {
    let mut meta_ids: Vec<String> = read_records()
        .into_iter()
        .filter(|record| record.has_tags(tags))
        .map(|record| record.meta.id.id)
        .collect();
    meta_ids.sort();
    meta_ids.dedup();
    meta_ids.truncate(limit);
    meta_ids
        .iter()
        .filter_map(|meta_id| load_anime(meta_id))
        .collect()
}

/// 读出数据目录里所有文件的轻量记录（标题 + 标签 + `meta.id`）。
fn read_records() -> Vec<TitleRecord> {
    std::fs::read_dir(AnimeNamespace::data_path())
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().extension().and_then(|e| e.to_str()) == Some("json"))
        .filter_map(|entry| read_titles(&entry.path()))
        .collect()
}

/// 按 `meta.id` 顺序读回这些番剧。
fn load_meta_ids(meta_ids: BTreeSet<String>, limit: usize) -> Vec<Anime> {
    meta_ids
        .into_iter()
        .take(limit)
        .filter_map(|meta_id| load_anime(&meta_id))
        .collect()
}

/// 按分数降序取前 `limit` 条，并读回对应的番剧。
fn rank(best: BTreeMap<String, f32>, limit: usize) -> Vec<Anime> {
    let mut hits = best.into_iter().collect::<Vec<_>>();
    hits.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
    hits.truncate(limit);
    hits.into_iter()
        .filter_map(|(meta_id, _)| load_anime(&meta_id))
        .collect()
}

fn load_anime(meta_id: &str) -> Option<Anime> {
    Anime::load_with_meta_id(&MetaId::with_id(meta_id.to_string())).ok()
}

/// 数据文件里 `search` 需要的那几个字段：`meta.id`、各语言标题、标签。
#[derive(serde::Deserialize)]
struct TitleRecord {
    meta: MetaIdOnly,
    #[serde(default, rename = "i18n")]
    i18n: BTreeMap<String, I18nTitle>,
    #[serde(default)]
    info: InfoTags,
}

#[derive(serde::Deserialize)]
struct MetaIdOnly {
    id: MetaIdString,
}

#[derive(serde::Deserialize)]
struct MetaIdString {
    id: String,
}

#[derive(serde::Deserialize)]
struct I18nTitle {
    title: Option<String>,
}

/// `info` 里只取 `tags`；老数据文件没有这个字段时是空列表。
#[derive(serde::Deserialize, Default)]
struct InfoTags {
    #[serde(default)]
    tags: Vec<String>,
}

impl TitleRecord {
    fn titles(&self) -> impl Iterator<Item = &str> {
        self.i18n.values().filter_map(|i18n| i18n.title.as_deref())
    }

    /// 这条记录是不是**同时**带着全部这些标签（`tags` 已小写化，标签也按小写比较）。
    ///
    /// 空列表不算匹配：那是「没有按标签筛选」，不该把所有条目都算进来。
    fn has_tags(&self, tags: &[String]) -> bool {
        !tags.is_empty()
            && tags.iter().all(|wanted| {
                self.info
                    .tags
                    .iter()
                    .any(|tag| tag.trim().to_lowercase() == *wanted)
            })
    }
}

/// 只从数据文件里读出标题与 `meta.id`，不解析整个 [`Anime`]。
///
/// 完整反序列化（i18n 简介、state、infobox 等）在 debug 构建下要慢两个数量级，
/// 而搜索只需要这两个字段。读不动或格式不对的文件直接跳过。
fn read_titles(path: &Path) -> Option<TitleRecord> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// 单个标题的匹配分数：完全相同 > 前缀 > 包含，同一档里标题越短越靠前。
///
/// 返回 `None` 表示不匹配（空 `query` 一律视为不匹配）。`title` 与 `query`
/// 都按小写比较。
#[must_use]
pub fn score_title(title: &str, query: &str) -> Option<f32> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return None;
    }
    let title = title.to_lowercase();
    let rank = if title == query {
        4.0
    } else if title.starts_with(&query) {
        3.0
    } else if title.contains(&query) {
        2.0
    } else {
        return None;
    };
    // 标题长度只用来打破同分，长度本身不会接近 f32 的精度上限。
    #[allow(clippy::cast_precision_loss)]
    let length_penalty = title.chars().count() as f32 / 1000.0;
    Some(rank - length_penalty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_prefers_exact_then_prefix_then_contains() {
        let exact = score_title("物语", "物语").unwrap();
        let prefix = score_title("物语系列", "物语").unwrap();
        let contains = score_title("〈物语〉系列", "物语").unwrap();
        assert!(exact > prefix, "{exact} > {prefix}");
        assert!(prefix > contains, "{prefix} > {contains}");
    }

    #[test]
    fn score_is_case_insensitive_and_misses_are_none() {
        assert!(score_title("steins;gate", "steins").is_some());
        assert!(score_title("Rick and Morty Season 1", "RICK").is_some());
        assert_eq!(score_title("Steins;Gate", "clannad"), None);
    }

    #[test]
    fn shorter_title_wins_at_the_same_rank() {
        let short = score_title("物语", "物语").unwrap();
        let long = score_title("物语系列第二季", "物语").unwrap();
        assert!(short > long, "{short} > {long}");
    }

    #[test]
    fn empty_query_is_not_scored() {
        assert_eq!(score_title("物语", ""), None);
    }

    /// 提示只在「用户以为在查索引、其实没查到」时出现。
    #[test]
    fn fallback_hints_cover_the_interesting_cases() {
        let index = Path::new("anime/.aki/anime-index.sqlite");

        // 索引不存在：一定提示，并给出重建命令。
        let hint = fallback_hint(&IndexLookup::Missing, 3, index).unwrap();
        assert!(hint.contains("not found"), "{hint}");
        assert!(hint.contains("build-index"), "{hint}");

        // 索引在、但标题表是空的：也提示。
        let hint = fallback_hint(&IndexLookup::Empty, 0, index).unwrap();
        assert!(hint.contains("no title entry"), "{hint}");

        // 索引没匹配到、文件扫到了：提示索引可能过期。
        let hint = fallback_hint(&IndexLookup::Hits(BTreeMap::new()), 2, index).unwrap();
        assert!(hint.contains("found 2 anime"), "{hint}");
        assert!(hint.contains("out of date"), "{hint}");

        // 索引没匹配到、文件也没扫到：这是「真的没有」，不是索引的问题。
        assert!(fallback_hint(&IndexLookup::Hits(BTreeMap::new()), 0, index).is_none());
        // 打不开时 is_available 已经报过原因，不再重复。
        assert!(fallback_hint(&IndexLookup::Unreadable, 0, index).is_none());

        // 标签侧同理：标签表不存在 / 是空的 → 提示重建；索引没这个标签但文件有 → 提示过期。
        assert!(
            tag_fallback_hint(&TagLookup::Missing, 0, index)
                .unwrap()
                .contains("not found")
        );
        assert!(
            tag_fallback_hint(&TagLookup::Empty, 0, index)
                .unwrap()
                .contains("no tag entry")
        );
        assert!(
            tag_fallback_hint(&TagLookup::Hits(BTreeSet::new()), 2, index)
                .unwrap()
                .contains("found 2 anime")
        );
        assert!(tag_fallback_hint(&TagLookup::Hits(BTreeSet::new()), 0, index).is_none());
    }

    /// 标签要先统一成「小写、去空白、去重、有序」再拿去查。
    #[test]
    fn tags_are_normalized_before_lookup() {
        let tags = [
            " 奇幻 ".to_string(),
            "奇幻".to_string(),
            String::new(),
            "TV".to_string(),
        ];
        assert_eq!(normalize_tags(&tags), vec!["tv", "奇幻"]);
        assert!(normalize_tags(&[]).is_empty());
        assert!(normalize_tags(&["   ".to_string()]).is_empty());
    }

    /// 轻量记录里的标签判断：全部命中才算匹配，空条件不算匹配。
    #[test]
    fn record_matches_all_requested_tags() {
        let record: TitleRecord = serde_json::from_str(
            r#"{
                "meta": {"id": {"id": "[2020-01-01] A"}},
                "info": {"tags": ["奇幻", "TV", "SHAFT"]}
            }"#,
        )
        .unwrap();

        let wanted = |tags: &[&str]| {
            tags.iter()
                .map(|tag| (*tag).to_string())
                .collect::<Vec<_>>()
        };
        assert!(record.has_tags(&wanted(&["奇幻"])));
        assert!(record.has_tags(&wanted(&["tv", "shaft"]))); // 大小写不敏感
        assert!(!record.has_tags(&wanted(&["奇幻", "恋爱"]))); // 「且」
        assert!(!record.has_tags(&[])); // 没有条件就不是标签搜索

        // 老数据文件没有 tags 字段：当成空列表，不会匹配任何标签。
        let old: TitleRecord = serde_json::from_str(
            r#"{"meta": {"id": {"id": "[2020-01-01] old"}}, "info": {"title": "old"}}"#,
        )
        .unwrap();
        assert!(!old.has_tags(&wanted(&["奇幻"])));
    }

    #[tokio::test]
    #[ignore = "cannot panic: only prints the hits from the local database"]
    async fn test_search() {
        for anime in search("物语", &[], 10, true).await {
            dbg!(anime.to_string());
        }
    }

    #[tokio::test]
    #[ignore = "cannot panic: only prints the anime with that tag from the local database"]
    async fn test_search_by_tag() {
        for anime in search("", &["奇幻".to_string()], 10, true).await {
            dbg!(anime.to_string());
        }
    }
}
