use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};

use super::super::Anime;

#[derive(Debug, thiserror::Error)]
pub enum ExportStateError {
    #[error("create output directory error")]
    CreateDir(#[source] std::io::Error),
    #[error("open sqlite database error")]
    Open(#[source] sqlx::Error),
    #[error("write sqlite database error")]
    Write(#[source] sqlx::Error),
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
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

/// 把全部番剧的 `state` 导出到 `output` 指定的 `SQLite` 文件，返回（写入的文件, 行数）。
///
/// `output` 为必填。每次运行都重建 `anime_state` 表，结果始终是当前状态的完整快照。
pub async fn export_state(output: &Path) -> Result<(PathBuf, usize), ExportStateError> {
    let path = output.to_path_buf();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(ExportStateError::CreateDir)?;
    }
    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(ExportStateError::Open)?;
    let count = write_state(&pool, Anime::iter_all()).await?;
    Ok((path, count))
}

/// 把 f32 分数取到 6 位小数，避免浮点误差。
fn real_score(value: f32) -> f64 {
    (f64::from(value) * 1_000_000.0).round() / 1_000_000.0
}

/// 把一批番剧的 state 写进已打开的库，返回写入行数。
pub(super) async fn write_state(
    pool: &SqlitePool,
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

    // PRAGMA / DROP / CREATE 都是不能带参数的裸语句，走 `raw_sql`。
    // `PRAGMA user_version` 不接受绑定参数，这里拼的是编译期常量，无注入风险。
    let pragma = format!("PRAGMA user_version = {SCHEMA_VERSION}");
    sqlx::raw_sql(sqlx::AssertSqlSafe(pragma))
        .execute(pool)
        .await
        .map_err(ExportStateError::Write)?;
    sqlx::raw_sql("DROP TABLE IF EXISTS anime_state")
        .execute(pool)
        .await
        .map_err(ExportStateError::Write)?;
    sqlx::raw_sql(CREATE_TABLE)
        .execute(pool)
        .await
        .map_err(ExportStateError::Write)?;

    let count = rows.len();
    // 用事务包住全部插入，否则每行都要等一次 fsync。
    let mut transaction = pool.begin().await.map_err(ExportStateError::Write)?;
    for anime in rows.values() {
        let state = &anime.state;
        let score = state.score();
        sqlx::query(INSERT_ROW)
            .bind(&anime.meta.id.id)
            .bind(anime.bangumi_id)
            .bind(&anime.info.title)
            .bind(anime.meta.data.title.as_deref())
            .bind(state.watch_status().as_str())
            .bind(i64::from(state.episodes_watched()))
            .bind(score.positive.map(real_score))
            .bind(score.negative.map(real_score))
            .bind(score.recommendation.map(i64::from))
            .bind(state.comment())
            .bind(state.watch_start_date().map(|date| date.to_string()))
            .bind(state.watch_finish_date().map(|date| date.to_string()))
            .execute(&mut *transaction)
            .await
            .map_err(ExportStateError::Write)?;
    }
    transaction
        .commit()
        .await
        .map_err(ExportStateError::Write)?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::anime::{AnimeInfo, AnimeScore, AnimeState, AnimeWatchStatus};
    use crate::{Language, Meta};
    use chrono::NaiveDate;

    /// 单连接的内存库。
    async fn in_memory() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap()
    }

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
                tags: Vec::new(),
            },
            BTreeMap::<Language, crate::anime::AnimeInternationalization>::new(),
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

    type StateRow = (
        String,
        Option<i64>,
        String,
        Option<String>,
        String,
        i64,
        Option<f64>,
        Option<f64>,
        Option<i64>,
        Option<String>,
        Option<String>,
        Option<String>,
    );

    #[tokio::test]
    async fn exports_every_column() {
        let pool = in_memory().await;
        let anime = fully_filled();
        let count = write_state(&pool, std::iter::once(anime)).await.unwrap();
        assert_eq!(count, 1);

        let row: StateRow = sqlx::query_as(
            "SELECT meta_id, bangumi_id, title, title_zh, watch_status, episodes_watched,
                    positive, negative, recommendation, comment, watch_start_date, watch_finish_date
             FROM anime_state",
        )
        .fetch_one(&pool)
        .await
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

        let version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[tokio::test]
    async fn missing_fields_become_null() {
        type Row = (
            Option<i64>,
            Option<String>,
            String,
            i64,
            Option<f64>,
            Option<f64>,
            Option<i64>,
            Option<String>,
            Option<String>,
            Option<String>,
        );

        let pool = in_memory().await;
        write_state(&pool, std::iter::once(mostly_empty()))
            .await
            .unwrap();

        let row: Row = sqlx::query_as(
            "SELECT bangumi_id, title_zh, watch_status, episodes_watched, positive,
                    negative, recommendation, comment, watch_start_date, watch_finish_date
             FROM anime_state",
        )
        .fetch_one(&pool)
        .await
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

    #[tokio::test]
    async fn rebuilding_replaces_rows_but_keeps_other_tables() {
        let pool = in_memory().await;
        sqlx::raw_sql("CREATE TABLE keep_me (id INTEGER); INSERT INTO keep_me VALUES (7);")
            .execute(&pool)
            .await
            .unwrap();

        write_state(&pool, [fully_filled(), mostly_empty()].into_iter())
            .await
            .unwrap();
        let first: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM anime_state")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(first, 2);

        // 再导出一次，且这次只有一条：应当是快照，不是追加。
        let count = write_state(&pool, std::iter::once(fully_filled()))
            .await
            .unwrap();
        assert_eq!(count, 1);
        let second: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM anime_state")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(second, 1);

        let kept: i64 = sqlx::query_scalar("SELECT id FROM keep_me")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(kept, 7);
    }

    #[tokio::test]
    async fn duplicated_meta_id_keeps_one_row() {
        let pool = in_memory().await;
        let count = write_state(&pool, [fully_filled(), fully_filled()].into_iter())
            .await
            .unwrap();
        assert_eq!(count, 1);
        let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM anime_state")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rows, 1);
    }
}
