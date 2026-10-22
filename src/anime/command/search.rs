use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use super::super::{Anime, MetaId, index};

/// 按标题匹配字符串（可用标签筛选），返回分数最高的前 `limit` 条。
///
/// 标题走 `.aki/anime-index.sqlite` 的 `titles` 表：把整张表扫一遍做字符串匹配，
/// 只对最终要返回的那几条去读数据文件。`tags` 非空时再用 `tags`/`anime_tags` 里的
/// 标签集合过滤，多个标签之间是「且」。
///
/// `query` 为空、只给 `tags` 时，就是「按标签列番剧」：按 `meta.id` 顺序（也就是
/// `[日期] 名字` 的日期顺序）返回，不走标题打分。
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
    let best = lookup_index(query).await;
    let hits = rank(filter_by_tags(best, tags).await, limit);
    if !hits.is_empty() || !verify_index {
        return hits;
    }
    // 索引里一条都没有，但被要求确认：重建索引后再查一次（索引可能落后于数据目录，
    // 比如手工放过文件、加了条目没重建索引）。
    if index::build().await.is_err() {
        return Vec::new();
    }
    rank(filter_by_tags(lookup_index(query).await, tags).await, limit)
}

async fn filter_by_tags(best: BTreeMap<String, f32>, tags: &[String]) -> BTreeMap<String, f32> {
    if tags.is_empty() {
        return best;
    }
    let hits = lookup_tags(tags).await;
    best.into_iter()
        .filter(|(meta_id, _)| hits.contains(meta_id))
        .collect()
}
/// 按标签找番剧（多个标签之间是「且」）。
///
/// 结果按 `meta.id` 排序（`[YYYY-MM-DD] 名字` 正好是日期顺序）。
async fn search_by_tags(tags: &[String], limit: usize, verify_index: bool) -> Vec<Anime> {
    let hits = lookup_tags(tags).await;
    if !hits.is_empty() {
        return load_meta_ids(hits, limit);
    }
    // 索引里没有，而又没要求确认：索引说了没有就是没有。
    if !verify_index {
        return Vec::new();
    }
    // 索引里没有、被要求确认：重建索引后再查一次（索引可能落后于数据目录）。
    if index::build().await.is_err() {
        return Vec::new();
    }
    load_meta_ids(lookup_tags(tags).await, limit)
}

/// 在索引的标题表上按标题匹配（匹配在 SQL 内完成，打分在 Rust 里做）。
async fn lookup_index(query: &str) -> BTreeMap<String, f32> {
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
    best
}

/// 查索引的标签表，返回命中的 `meta.id` 集合（多个标签取交集）。
async fn lookup_tags(tags: &[String]) -> BTreeSet<String> {
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
    hits.unwrap_or_default()
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
