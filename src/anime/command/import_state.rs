use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::super::{Anime, AnimeScore, AnimeState, AnimeWatchStatus};
use crate::CONFIG;

/// 校验失败时最多在错误里列出多少行，避免刷屏。
const MAX_REPORTED_ISSUES: usize = 10;

#[derive(Debug, thiserror::Error)]
pub enum ImportStateError {
    #[error("input database not found: {}", .0.display())]
    NotFound(PathBuf),
    #[error("open sqlite database error")]
    Open(#[source] rusqlite::Error),
    #[error("read sqlite database error")]
    Read(#[source] rusqlite::Error),
    #[error("{} row(s) are invalid, nothing was written: {}", .0.len(), format_issues(.0))]
    InvalidRows(Vec<String>),
    #[error("write anime file error")]
    WriteAnime(#[source] std::io::Error),
    #[error("remove renamed anime file error")]
    RemoveRenamed(#[source] std::io::Error),
    #[error("serialize anime error")]
    SerializeAnime(#[source] serde_json::Error),
}

fn format_issues(issues: &[String]) -> String {
    let mut parts = issues
        .iter()
        .take(MAX_REPORTED_ISSUES)
        .cloned()
        .collect::<Vec<_>>();
    if issues.len() > MAX_REPORTED_ISSUES {
        parts.push(format!("… 另有 {} 行", issues.len() - MAX_REPORTED_ISSUES));
    }
    parts.join("; ")
}

/// 导入结果。
#[derive(Debug, Default)]
pub struct ImportStateResult {
    /// 被改动（`--dry-run` 时是「将会被改动」）的 `meta.id`。
    pub updated: Vec<String>,
    /// DB 里的 state 与文件里完全一致、不需要动的条目数。
    pub unchanged: usize,
    /// DB 里有、但数据目录里找不到对应条目的 `meta.id`。
    pub unknown: Vec<String>,
}

const SELECT_ROWS: &str = "
SELECT meta_id, watch_status, episodes_watched, positive, negative,
       recommendation, comment, watch_start_date, watch_finish_date
FROM anime_state";

/// 从数据库读回 `state` 并更新对应的番剧数据文件。
///
/// `input` 为 `None` 时读 [`CONFIG.anime_state_db_path()`](crate::CONFIG)。
/// 只改 `state`，`info` / `i18n` / `meta` / `bangumi_id` 一律不碰；`state`
/// 与文件里完全一致的条目会被跳过，不会白写一遍文件。
///
/// 所有行先校验、再落盘：只要有一行不合法就整体不写，不会留下改了一半的数据。
/// `dry_run` 为 `true` 时只报告会改哪些，不写任何文件。
pub fn import_state(
    input: Option<&Path>,
    dry_run: bool,
) -> Result<ImportStateResult, ImportStateError> {
    let path = input.map_or_else(
        || CONFIG.anime_state_db_path().to_path_buf(),
        Path::to_path_buf,
    );
    if !path.exists() {
        return Err(ImportStateError::NotFound(path));
    }
    let connection = Connection::open(&path).map_err(ImportStateError::Open)?;
    let states = read_states(&connection)?;
    merge(
        super::super::Anime::iter_all_with_path(),
        &states,
        |loaded_path, anime| {
            if dry_run {
                return Ok(());
            }
            write_anime(loaded_path, anime)
        },
    )
}

/// 读出全部行的 `state`。任何一行不合法都会返回 [`ImportStateError::InvalidRows`]。
fn read_states(connection: &Connection) -> Result<BTreeMap<String, AnimeState>, ImportStateError> {
    let mut statement = connection
        .prepare(SELECT_ROWS)
        .map_err(ImportStateError::Read)?;
    let mut rows = statement.query([]).map_err(ImportStateError::Read)?;

    let mut states = BTreeMap::new();
    let mut issues = Vec::new();
    while let Some(row) = rows.next().map_err(ImportStateError::Read)? {
        let meta_id: String = row.get(0).map_err(ImportStateError::Read)?;
        match parse_state(row) {
            Ok(state) => {
                states.insert(meta_id, state);
            }
            Err(issue) => issues.push(format!("{meta_id}: {issue}")),
        }
    }
    if issues.is_empty() {
        Ok(states)
    } else {
        Err(ImportStateError::InvalidRows(issues))
    }
}

/// 把一行解析成 [`AnimeState`]，出错时返回可以直接读的原因。
///
/// 这里刻意不用 `AnimeScore::new()`：它会静默把越界值 clamp 掉，而导入时
/// 越界应该被当成错误报出来，而不是悄悄改成另一个数。
fn parse_state(row: &rusqlite::Row<'_>) -> Result<AnimeState, String> {
    let read = |index: usize| {
        row.get::<_, Option<String>>(index)
            .map_err(|e| e.to_string())
    };

    let status = read(1)?.ok_or("watch_status 不能为空")?;
    let watch_status = AnimeWatchStatus::try_from(status.as_str())
        .map_err(|_| format!("未知的 watch_status: {status}"))?;

    let episodes = row
        .get::<_, Option<i64>>(2)
        .map_err(|e| e.to_string())?
        .ok_or("episodes_watched 不能为空")?;
    let episodes_watched =
        u16::try_from(episodes).map_err(|_| format!("episodes_watched 超出范围: {episodes}"))?;

    let positive = ratio(row, 3, "positive")?;
    let negative = ratio(row, 4, "negative")?;
    let recommendation = row
        .get::<_, Option<i64>>(5)
        .map_err(|e| e.to_string())?
        .map(|value| {
            u8::try_from(value)
                .ok()
                .filter(|value| (1..=5).contains(value))
                .ok_or_else(|| format!("recommendation 应在 1~5，实际是 {value}"))
        })
        .transpose()?;

    Ok(AnimeState::new(
        watch_status,
        episodes_watched,
        AnimeScore {
            positive,
            negative,
            recommendation,
        },
        read(6)?,
        date(row, 7, "watch_start_date")?,
        date(row, 8, "watch_finish_date")?,
    ))
}

/// 读一个 0~1 的比例字段，越界即报错。
// 库里的分数是 REAL(f64)，字段是 f32，这里的有意收窄是预期的。
#[allow(clippy::cast_possible_truncation)]
fn ratio(row: &rusqlite::Row<'_>, index: usize, name: &str) -> Result<Option<f32>, String> {
    row.get::<_, Option<f64>>(index)
        .map_err(|e| e.to_string())?
        .map(|value| {
            if (0.0..=1.0).contains(&value) {
                Ok(value as f32)
            } else {
                Err(format!("{name} 应在 0~1，实际是 {value}"))
            }
        })
        .transpose()
}

/// 读一个 ISO 日期字段。
fn date(
    row: &rusqlite::Row<'_>,
    index: usize,
    name: &str,
) -> Result<Option<chrono::NaiveDate>, String> {
    row.get::<_, Option<String>>(index)
        .map_err(|e| e.to_string())?
        .map(|value| {
            chrono::NaiveDate::parse_from_str(&value, "%Y-%m-%d")
                .map_err(|_| format!("{name} 不是合法的 YYYY-MM-DD 日期: {value}"))
        })
        .transpose()
}

/// 把 DB 里的 state 合并到番剧上；只有确实变了的条目才会调用 `on_change`。
///
/// 每一项都带上「这条数据是从哪个文件读出来的」，`on_change` 拿到它才能原地
/// 改写、并在文件名不规范时清理旧文件。与 [`CONFIG`] 无关，可脱离真实数据目录测试。
fn merge(
    entries: impl Iterator<Item = (PathBuf, Anime)>,
    states: &BTreeMap<String, AnimeState>,
    mut on_change: impl FnMut(&Path, &Anime) -> Result<(), ImportStateError>,
) -> Result<ImportStateResult, ImportStateError> {
    let mut result = ImportStateResult::default();
    let mut seen = BTreeSet::new();
    for (path, mut anime) in entries {
        let meta_id = anime.meta.id.id.clone();
        seen.insert(meta_id.clone());
        let Some(state) = states.get(&meta_id) else {
            continue;
        };
        if &anime.state == state {
            result.unchanged += 1;
            continue;
        }
        anime.state = state.clone();
        on_change(&path, &anime)?;
        result.updated.push(meta_id);
    }
    result.unknown = states
        .keys()
        .filter(|meta_id| !seen.contains(*meta_id))
        .cloned()
        .collect();
    Ok(result)
}

/// 先写临时文件再改名覆盖，避免写到一半失败把已有的数据文件写坏。
///
/// 目标是 `meta.id` 推算出的规范文件名。如果数据其实是从另一个文件名读出来的
/// （手工改过名、或是从别处复制来的），写完后会把那个旧文件删掉——否则同一个
/// `meta.id` 会在数据目录里留下两份。
fn write_anime(loaded_path: &Path, anime: &Anime) -> Result<(), ImportStateError> {
    write_anime_at(&anime.meta.id.json_data_path(), loaded_path, anime)
}

/// 真正的落盘逻辑。`target` 由调用方给出，测试才能脱离真实数据目录验证。
fn write_anime_at(
    target: &Path,
    loaded_path: &Path,
    anime: &Anime,
) -> Result<(), ImportStateError> {
    let temp = target.with_extension("json.tmp");
    {
        let file = File::create(&temp).map_err(ImportStateError::WriteAnime)?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer_pretty(&mut writer, anime)
            .map_err(ImportStateError::SerializeAnime)?;
        writer.flush().map_err(ImportStateError::WriteAnime)?;
    }
    fs::rename(&temp, target).map_err(ImportStateError::WriteAnime)?;

    if loaded_path != target {
        fs::remove_file(loaded_path).map_err(ImportStateError::RemoveRenamed)?;
        eprintln!(
            "Warning: '{}' was not at its canonical path, written to '{}' and the old file was removed",
            loaded_path.display(),
            target.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::{AnimeInfo, AnimeInfoInternationalization, Language, Meta};
    use chrono::NaiveDate;

    fn sample(id: &str, state: AnimeState) -> Anime {
        Anime::new(
            Meta::with_id(id.to_string(), Some(format!("{id} 中文名"))),
            AnimeInfo {
                title: format!("{id} original"),
                summary: None,
                platform: None,
                air_date: NaiveDate::from_ymd_opt(2020, 1, 1),
                season: None,
                episodes: Some(12),
            },
            BTreeMap::<Language, AnimeInfoInternationalization>::new(),
            state,
            Some(265),
        )
    }

    fn filled_state() -> AnimeState {
        AnimeState::new(
            AnimeWatchStatus::Watched,
            12,
            AnimeScore {
                positive: Some(0.8),
                negative: Some(0.2),
                recommendation: Some(5),
            },
            Some("很好看".to_string()),
            NaiveDate::from_ymd_opt(2024, 3, 1),
            NaiveDate::from_ymd_opt(2024, 3, 20),
        )
    }

    fn empty_state() -> AnimeState {
        AnimeState::new(
            AnimeWatchStatus::Plan,
            0,
            AnimeScore::default(),
            None,
            None,
            None,
        )
    }

    fn exported(states: &[(&str, AnimeState)]) -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        let animes = states
            .iter()
            .map(|(id, state)| sample(id, state.clone()))
            .collect::<Vec<_>>();
        super::super::export_state::write_state(&connection, animes.into_iter()).unwrap();
        connection
    }

    #[test]
    fn round_trips_with_export() {
        let connection = exported(&[("a", filled_state()), ("b", empty_state())]);
        let states = read_states(&connection).unwrap();
        assert_eq!(states.len(), 2);
        assert_eq!(states["a"], filled_state());
        assert_eq!(states["b"], empty_state());
    }

    #[test]
    fn merge_only_touches_changed_rows() {
        let mut states = BTreeMap::new();
        states.insert("a".to_string(), filled_state());
        states.insert("b".to_string(), empty_state());
        // 与 DB 不同的那一条；b 保持与 DB 一致。
        let entries = vec![
            (PathBuf::from("a.json"), sample("a", empty_state())),
            (PathBuf::from("b.json"), sample("b", empty_state())),
        ];

        let mut written = Vec::new();
        let result = merge(entries.into_iter(), &states, |_, anime| {
            written.push(anime.meta.id.id.clone());
            Ok(())
        })
        .unwrap();

        assert_eq!(result.updated, vec!["a".to_string()]);
        assert_eq!(result.unchanged, 1);
        assert_eq!(written, vec!["a".to_string()]);
    }

    #[test]
    fn unknown_meta_ids_are_reported() {
        let mut states = BTreeMap::new();
        states.insert("a".to_string(), empty_state());
        states.insert("gone".to_string(), empty_state());

        let result = merge(
            std::iter::once((PathBuf::from("a.json"), sample("a", empty_state()))),
            &states,
            |_, _| Ok(()),
        )
        .unwrap();

        assert_eq!(result.updated, Vec::<String>::new());
        assert_eq!(result.unchanged, 1);
        assert_eq!(result.unknown, vec!["gone".to_string()]);
    }

    /// 数据文件如果不在规范路径上（手工改过名、或从别处复制来的），改写后
    /// 必须把旧文件清掉，否则同一个 `meta.id` 会在目录里留下两份。
    #[test]
    fn writes_canonically_and_removes_the_outdated_file() {
        let dir = std::env::temp_dir().join(format!("aki-import-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&dir).unwrap();
        let loaded = dir.join("wrong-name.json");
        std::fs::write(&loaded, b"{}").unwrap();
        let target = dir.join("canonical.json");

        let anime = sample("id", filled_state());
        write_anime_at(&target, &loaded, &anime).unwrap();

        assert!(target.exists(), "应当写到规范路径");
        assert!(!loaded.exists(), "旧文件名应当被清掉");
        assert!(!dir.join("canonical.json.tmp").exists(), "临时文件不应残留");
        let reloaded = Anime::load(&target).unwrap();
        assert_eq!(reloaded.state, filled_state());

        // 文件名本来就规范时，只是原地覆盖。
        write_anime_at(&target, &target, &anime).unwrap();
        assert!(target.exists());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rejects_rows_that_would_violate_the_schema() {
        let connection = Connection::open_in_memory().unwrap();
        super::super::export_state::write_state(
            &connection,
            std::iter::once(sample("ok", empty_state())),
        )
        .unwrap();
        connection
            .execute_batch(
                "INSERT INTO anime_state (meta_id, title, watch_status, episodes_watched, positive)
                 VALUES ('bad-status', 'x', 'nonsense', 1, 0.5);
                 INSERT INTO anime_state (meta_id, title, watch_status, episodes_watched, positive)
                 VALUES ('bad-score', 'x', 'watched', 1, 9.0);
                 INSERT INTO anime_state (meta_id, title, watch_status, episodes_watched)
                 VALUES ('huge-episodes', 'x', 'watched', 70000);
                 INSERT INTO anime_state (meta_id, title, watch_status, episodes_watched, watch_start_date)
                 VALUES ('bad-date', 'x', 'watched', 1, '2024/03/01');",
            )
            .unwrap();

        let error = read_states(&connection).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("bad-status"), "{message}");
        assert!(message.contains("nonsense"), "{message}");
        assert!(message.contains("bad-score"), "{message}");
        assert!(message.contains("huge-episodes"), "{message}");
        assert!(message.contains("bad-date"), "{message}");
        // 合法的行也会被整体拒绝——不会写一半。
        assert_eq!(format_issues(&["a".to_string(), "b".to_string()]), "a; b");
    }
}
