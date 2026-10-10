use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};

use super::super::Anime;
use crate::CONFIG;

#[derive(Debug, thiserror::Error)]
pub enum ExportStateError {
    #[error("create output directory error")]
    CreateDir(#[source] std::io::Error),
    #[error("open sqlite database error")]
    Open(#[source] rusqlite::Error),
    #[error("write sqlite database error")]
    Write(#[source] rusqlite::Error),
}

/// schema 版本，写在 `PRAGMA user_version` 里，便于以后改结构时判断。
const SCHEMA_VERSION: i64 = 1;

const CREATE_TABLE: &str = "
CREATE TABLE anime_state (
    meta_id           TEXT PRIMARY KEY,
    bangumi_id        INTEGER,
    title             TEXT NOT NULL,
    title_zh          TEXT,
    watch_status      TEXT NOT NULL,
    episodes_watched  INTEGER NOT NULL,
    positive          REAL,
    negative          REAL,
    recommendation    INTEGER,
    comment           TEXT,
    watch_start_date  TEXT,
    watch_finish_date TEXT
)";

const INSERT_ROW: &str = "
INSERT INTO anime_state (
    meta_id, bangumi_id, title, title_zh, watch_status, episodes_watched,
    positive, negative, recommendation, comment, watch_start_date, watch_finish_date
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)";

/// 把全部番剧的 `state` 导出到 SQLite，返回（实际写入的文件, 行数）。
///
/// `output` 为 `None` 时写到 [`CONFIG.anime_state_db_path()`](crate::CONFIG)。
/// 每次运行都会重建 `anime_state` 表（`DROP TABLE IF EXISTS` + `CREATE`），
/// 因此结果始终是当前状态的完整快照；库里其他表不受影响。
///
/// 分数按原样导出，不做范围裁剪——数据超出了字段约定的 0~1 也能看出来。
pub fn export_state(output: Option<&Path>) -> Result<(PathBuf, usize), ExportStateError> {
    let path = output.map_or_else(
        || CONFIG.anime_state_db_path().to_path_buf(),
        Path::to_path_buf,
    );
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(ExportStateError::CreateDir)?;
    }
    let connection = Connection::open(&path).map_err(ExportStateError::Open)?;
    let count = write_state(&connection, Anime::iter_all())?;
    Ok((path, count))
}

/// f32 的 `0.8` 转成 f64 会变成 `0.800000011920929`。取到 6 位小数，让库里的
/// 数值和 JSON 里写的一致，`WHERE positive = 0.8` 这类查询才不会意外落空。
fn real_score(value: f32) -> f64 {
    (f64::from(value) * 1_000_000.0).round() / 1_000_000.0
}

/// 把一批番剧的 state 写进已经打开的库，返回写入行数。
///
/// `pub(super)` 是为了让 `import_state` 的测试能直接用它验证「导出↔导入」一致。
pub(super) fn write_state(
    connection: &Connection,
    animes: impl Iterator<Item = Anime>,
) -> Result<usize, ExportStateError> {
    // `meta.id` 是这条数据的身份，正常不会重复（重复的话文件名也会撞）。
    // 真出现了就保留最后一个，并明确提示，而不是悄悄少导一条。
    let mut rows = BTreeMap::new();
    let mut total = 0usize;
    for anime in animes {
        total += 1;
        rows.insert(anime.meta.id.id.clone(), anime);
    }
    if rows.len() < total {
        eprintln!(
            "[?] {} anime entries share a meta.id with another file, \
             only the last of each is exported",
            total - rows.len()
        );
    }

    connection
        .execute_batch(&format!(
            "PRAGMA user_version = {SCHEMA_VERSION};
             DROP TABLE IF EXISTS anime_state;
             {CREATE_TABLE};"
        ))
        .map_err(ExportStateError::Write)?;

    let mut statement = connection
        .prepare(INSERT_ROW)
        .map_err(ExportStateError::Write)?;
    let count = rows.len();
    {
        // 用事务包住全部插入，否则每行都要等一次 fsync。
        let transaction = connection
            .unchecked_transaction()
            .map_err(ExportStateError::Write)?;
        for anime in rows.values() {
            let state = &anime.state;
            let score = state.score();
            statement
                .execute(params![
                    &anime.meta.id.id,
                    anime.bangumi_id,
                    &anime.info.title,
                    anime.meta.data.title.as_deref(),
                    state.watch_status().as_str(),
                    i64::from(state.episodes_watched()),
                    score.positive.map(real_score),
                    score.negative.map(real_score),
                    score.recommendation.map(i64::from),
                    state.comment(),
                    state.watch_start_date().map(|date| date.to_string()),
                    state.watch_finish_date().map(|date| date.to_string()),
                ])
                .map_err(ExportStateError::Write)?;
        }
        transaction.commit().map_err(ExportStateError::Write)?;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::anime::{AnimeInfo, AnimeScore, AnimeState, AnimeWatchStatus};
    use crate::{Language, Meta};
    use chrono::NaiveDate;

    fn sample(id: &str, bangumi_id: Option<i64>, state: AnimeState) -> Anime {
        Anime::new(
            Meta::with_id(id.to_string(), Some(format!("{id} 中文名"))),
            AnimeInfo {
                title: format!("{id} original"),
                summary: Some("summary".to_string()),
                platform: Some("TV".to_string()),
                air_date: NaiveDate::from_ymd_opt(2020, 1, 1),
                season: None,
                episodes: Some(12),
            },
            BTreeMap::<Language, crate::anime::AnimeInfoInternationalization>::new(),
            state,
            bangumi_id,
        )
    }

    fn fully_filled() -> Anime {
        sample(
            "[2020-01-01] full",
            Some(265),
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
            ),
        )
    }

    fn mostly_empty() -> Anime {
        sample(
            "[2021-02-02] empty",
            None,
            AnimeState::new(
                AnimeWatchStatus::Plan,
                0,
                AnimeScore::default(),
                None,
                None,
                None,
            ),
        )
    }

    #[test]
    fn exports_every_column() {
        let connection = Connection::open_in_memory().unwrap();
        let anime = fully_filled();
        let count = write_state(&connection, std::iter::once(anime)).unwrap();
        assert_eq!(count, 1);

        let row = connection
            .query_row(
                "SELECT meta_id, bangumi_id, title, title_zh, watch_status, episodes_watched,
                        positive, negative, recommendation, comment, watch_start_date, watch_finish_date
                 FROM anime_state",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, Option<f64>>(6)?,
                        row.get::<_, Option<f64>>(7)?,
                        row.get::<_, Option<i64>>(8)?,
                        row.get::<_, Option<String>>(9)?,
                        row.get::<_, Option<String>>(10)?,
                        row.get::<_, Option<String>>(11)?,
                    ))
                },
            )
            .unwrap();

        assert_eq!(row.0, "[2020-01-01] full");
        assert_eq!(row.1, Some(265));
        assert_eq!(row.2, "[2020-01-01] full original");
        assert_eq!(row.3.as_deref(), Some("[2020-01-01] full 中文名"));
        assert_eq!(row.4, "watched");
        assert_eq!(row.5, 12);
        // 必须正好是 0.8，而不是 f32 转 f64 带出来的 0.800000011920929
        assert_eq!(row.6.unwrap().to_bits(), 0.8f64.to_bits());
        assert_eq!(row.7.unwrap().to_bits(), 0.2f64.to_bits());
        assert_eq!(row.8, Some(5));
        assert_eq!(row.9.as_deref(), Some("很好看"));
        assert_eq!(row.10.as_deref(), Some("2024-03-01"));
        assert_eq!(row.11.as_deref(), Some("2024-03-20"));

        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn missing_fields_become_null() {
        let connection = Connection::open_in_memory().unwrap();
        write_state(&connection, std::iter::once(mostly_empty())).unwrap();

        let row = connection
            .query_row(
                "SELECT bangumi_id, title_zh, watch_status, episodes_watched, positive,
                        negative, recommendation, comment, watch_start_date, watch_finish_date
                 FROM anime_state",
                [],
                |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, Option<f64>>(4)?,
                        row.get::<_, Option<f64>>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, Option<String>>(9)?,
                    ))
                },
            )
            .unwrap();

        assert_eq!(row.0, None);
        assert_eq!(row.1.as_deref(), Some("[2021-02-02] empty 中文名"));
        assert_eq!(row.2, "plan");
        assert_eq!(row.3, 0);
        assert_eq!(row.4, None);
        assert_eq!(row.5, None);
        assert_eq!(row.6, None);
        assert_eq!(row.7, None);
        assert_eq!(row.8, None);
        assert_eq!(row.9, None);
    }

    #[test]
    fn rebuilding_replaces_rows_but_keeps_other_tables() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("CREATE TABLE keep_me (id INTEGER); INSERT INTO keep_me VALUES (7);")
            .unwrap();

        write_state(&connection, [fully_filled(), mostly_empty()].into_iter()).unwrap();
        let first: i64 = connection
            .query_row("SELECT COUNT(*) FROM anime_state", [], |row| row.get(0))
            .unwrap();
        assert_eq!(first, 2);

        // 再导出一次，且这次只有一条：应当是快照，不是追加。
        let count = write_state(&connection, std::iter::once(fully_filled())).unwrap();
        assert_eq!(count, 1);
        let second: i64 = connection
            .query_row("SELECT COUNT(*) FROM anime_state", [], |row| row.get(0))
            .unwrap();
        assert_eq!(second, 1);

        let kept: i64 = connection
            .query_row("SELECT id FROM keep_me", [], |row| row.get(0))
            .unwrap();
        assert_eq!(kept, 7);
    }

    #[test]
    fn duplicated_meta_id_keeps_one_row() {
        let connection = Connection::open_in_memory().unwrap();
        let count = write_state(&connection, [fully_filled(), fully_filled()].into_iter()).unwrap();
        assert_eq!(count, 1);
        let rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM anime_state", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 1);
    }
}
