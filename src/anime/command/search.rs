use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::Path;

use super::super::{Anime, MetaId, index::AnimeIndex};
use crate::CONFIG;

/// 按标题匹配字符串，返回分数最高的前 `limit` 条。
///
/// 走 `.aki/fjall-index/` 的 `title-to-meta-id` keyspace：把整张表扫一遍做字符串
/// 匹配，只对最终要返回的那几条去读数据文件。
///
/// 索引不存在、打不开、或者里面还没有标题（没跑过 `build-index`）时退回扫数据
/// 目录，这样 `search` 在没有索引时依然可用，只是慢一些。
#[must_use]
pub fn search(query: &str, limit: usize) -> Vec<Anime> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    // 索引目录还不存在时直接扫文件：既不为了读一次标题去建一个空数据库，
    // 也不会把「没建索引」表现成「什么都没搜到」。
    if CONFIG.anime_fjall_index_path().exists()
        && let Some(index) = AnimeIndex::open_or_warn()
        && let Some(hits) = search_index(&index, query, limit)
    {
        return hits;
    }
    search_files(query, limit)
}

/// 在索引的标题表上找候选，再读回这些番剧。
///
/// 表里一条标题都没有时返回 `None`，让调用方退回扫文件。
fn search_index(index: &AnimeIndex, query: &str, limit: usize) -> Option<Vec<Anime>> {
    let mut entries = index.title_entries().peekable();
    entries.peek()?;
    let query = query.to_lowercase();
    // 同一个 `meta.id` 有多个标题时取最高分。
    let mut best: BTreeMap<String, f32> = BTreeMap::new();
    for (title, meta_id) in entries {
        let Some(score) = score_title(&title, &query) else {
            continue;
        };
        match best.get(&meta_id) {
            Some(&existing) if existing >= score => {}
            _ => {
                best.insert(meta_id, score);
            }
        }
    }
    Some(rank(best, limit))
}

/// 遍历数据目录逐个标题匹配（没有索引时的退路）。
///
/// 只读每个文件里的标题与 `meta.id`，不解析整个 [`Anime`]，因此不需要为了搜一次
/// 标题把 state / i18n 简介 / infobox 这些大块数据全部反序列化。
fn search_files(query: &str, limit: usize) -> Vec<Anime> {
    let query = query.to_lowercase();
    let mut best: BTreeMap<String, f32> = BTreeMap::new();
    for entry in std::fs::read_dir(CONFIG.anime_data_path())
        .ok()
        .into_iter()
        .flatten()
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let Some(record) = read_titles(&path) else {
            continue;
        };
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
    rank(best, limit)
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

/// 数据文件里 `search` 需要的那几个字段。
#[derive(serde::Deserialize)]
struct TitleRecord {
    meta: MetaIdOnly,
    info: InfoTitle,
    #[serde(default, rename = "i18n")]
    i18n: BTreeMap<String, I18nTitle>,
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
struct InfoTitle {
    title: String,
}

#[derive(serde::Deserialize)]
struct I18nTitle {
    title: Option<String>,
}

impl TitleRecord {
    fn titles(&self) -> impl Iterator<Item = &str> {
        self.i18n.values().filter_map(|i18n| i18n.title.as_deref())
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

    #[test]
    fn test_search() {
        for anime in search("物语", 10) {
            dbg!(anime.to_string());
        }
    }
}
