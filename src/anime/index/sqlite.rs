//! 番剧索引：`root/{anime.path}/.aki/anime-index.sqlite` 下的一个 SQLite 数据库。
//!
//! 连接池放在 [`POOL`]（`LazyLock`）里进程内共享，各命令不再各自开关数据库。
//! 三张表：
//!
//! | 表 | 列 | 用途 |
//! |----|----|----|
//! | `anime` | `id`(自增), `meta_id`, `bangumi_id` | 自增 id ↔ meta-id，`meta_id`/`bangumi_id` 带索引可反查 id |
//! | `id_title` | `id`(FK), `language`, `title` | 条目 id ↔ (language, title) |
//! | `id_tag` | `id`(FK), `tag` | 条目 id ↔ tag，`tag` 带索引 |
//!
//! 一条番剧有几个标题就在 `id_title` 里占几行，有几个标签就在 `id_tag` 里占几行。
//! 从 tag 反查 meta-id 时用 `id_tag` join `anime`（或按 id 二次查询）。
//! 索引是随时可以从数据目录重建的缓存：打不开就退化成「没有索引」，调用方回退到
//! 遍历数据文件。

use std::collections::HashSet;
use std::sync::LazyLock;

use sqlx::Transaction;
use sqlx::sqlite::{Sqlite, SqliteConnectOptions, SqlitePool, SqlitePoolOptions};

use super::super::{Anime, AnimeNamespace};
use crate::Language;

/// 进程内共享的 SQLite 连接池。SQLite 是单写者，连接数设 1 就够；打不开（目录建不出、
/// 数据库打不开）时置 `None`，所有查询退化成「没有索引」。
static POOL: LazyLock<Option<SqlitePool>> = LazyLock::new(init_pool);

/// [`build`] 的返回：各写了多少条。
pub struct BuildResult {
    pub bangumi_id_to_meta_id: usize,
    pub title_to_meta_id: usize,
    pub tag_to_meta_id: usize,
    pub errors: Vec<sqlx::Error>,
}

/// 初始化全局连接池：建目录、开库、建表。
///
/// 连接是懒建立的（`connect_lazy_with`），但建表要先真正连一次，所以这里另起一个
/// tokio runtime 同步把表结构建好，保证 [`POOL`] 首次被访问时立刻可用。
fn init_pool() -> Option<SqlitePool> {
    let path = AnimeNamespace::sqlite_index_path();
    if let Some(parent) = path.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        eprintln!(
            "[?] failed to create anime index directory '{}': {error}",
            parent.display()
        );
        return None;
    }
    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true)
        .foreign_keys(true);
    // `connect_lazy_with` 不立即建连，直接返回连接池；真正的失败（文件打不开等）
    // 会延迟到首次查询时暴露，由下面的建表兜住。
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_lazy_with(options);
    // 建表要真正连一次库，得在异步上下文里做；但 `init_pool` 可能已在某个 runtime 的
    // 线程上被调用（各命令用 `block_on` 包了一层），再 `Runtime::new().block_on()` 会
    // panic（不能嵌套 runtime）。于是另起一个线程建表，同步 `join` 等它完成。
    let schema_pool = pool.clone();
    let schema = std::thread::spawn(move || -> Result<(), sqlx::Error> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|error| sqlx::Error::Configuration(error.into()))?;
        runtime.block_on(initialize_schema(&schema_pool))
    })
    .join();
    match schema {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            eprintln!(
                "[?] failed to initialize anime index '{}': {error}",
                path.display()
            );
            return None;
        }
        Err(_) => {
            eprintln!(
                "[?] failed to initialize anime index '{}': schema thread panicked",
                path.display()
            );
            return None;
        }
    }
    Some(pool)
}

/// 建表（幂等，`IF NOT EXISTS`）。SQLite 不允许多语句一次执行，逐条来。
async fn initialize_schema(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS anime (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            meta_id TEXT NOT NULL UNIQUE,
            bangumi_id INTEGER UNIQUE
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS id_title (
            id INTEGER NOT NULL REFERENCES anime(id) ON DELETE CASCADE,
            language TEXT NOT NULL,
            title TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS id_tag (
            id INTEGER NOT NULL REFERENCES anime(id) ON DELETE CASCADE,
            tag TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_id_tag_tag ON id_tag(tag)")
        .execute(pool)
        .await?;
    Ok(())
}

/// 索引是否可用（全局连接池 [`POOL`] 是否已成功初始化）。
///
/// 索引是可重建的缓存，不该因为它打不开就让真正的增删改失败；初始化失败的警告
/// 已在 [`init_pool`] 里打印过，这里只返回 `false`。
#[must_use]
pub fn is_available() -> bool {
    POOL.as_ref().is_some()
}

/// 按 `bangumi_id` 查 `meta_id`。
#[must_use]
pub async fn get_meta_id_of_bangumi_id(bangumi_id: i64) -> Option<String> {
    let pool = POOL.as_ref()?;
    let (meta_id,): (String,) = sqlx::query_as("SELECT meta_id FROM anime WHERE bangumi_id = ?")
        .bind(bangumi_id)
        .fetch_optional(pool)
        .await
        .ok()??;
    Some(meta_id)
}

/// 某个标签下的全部 `meta_id`（标签大小写不敏感、首尾空白忽略）。
///
/// `id_tag` 里按 `tag` 定位到条目 id，再 join `anime` 取 `meta_id`。
#[must_use]
pub async fn meta_ids_of_tag(tag: &str) -> Vec<String> {
    let Some(pool) = POOL.as_ref() else {
        return Vec::new();
    };
    let normalized = tag.trim().to_lowercase();
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT a.meta_id
         FROM id_tag t
         JOIN anime a ON a.id = t.id
         WHERE t.tag = ?",
    )
    .bind(normalized)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    rows.into_iter().map(|(meta_id,)| meta_id).collect()
}

/// 标签表里有没有东西（用来区分「索引里真的没有这个标签」和「这份索引是加标签
/// 之前建的」）。
#[must_use]
pub async fn has_tag_entries() -> bool {
    let Some(pool) = POOL.as_ref() else {
        return false;
    };
    let (exists,): (i64,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM id_tag LIMIT 1)")
        .fetch_one(pool)
        .await
        .unwrap_or((0,));
    exists != 0
}

/// 遍历整个 `id_title` 表，返回 `(标题, meta.id)`。
#[must_use]
pub async fn title_entries() -> Vec<(String, String)> {
    let Some(pool) = POOL.as_ref() else {
        return Vec::new();
    };
    sqlx::query_as(
        "SELECT t.title, a.meta_id
         FROM id_title t
         JOIN anime a ON a.id = t.id",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

/// 把一条番剧写进索引（单事务：`anime`、`id_title`、`id_tag` 要么全成要么全不成）。
pub async fn set_anime_index(anime: &Anime) -> Result<(), sqlx::Error> {
    let pool = POOL.as_ref().ok_or(sqlx::Error::PoolClosed)?;
    insert_anime(pool, anime).await.map(|_| ())
}

/// [`set_anime_index`] 的容错版，失败只打印警告。
pub async fn set_anime_index_or_warn(anime: &Anime) {
    if let Err(error) = set_anime_index(anime).await {
        eprintln!("[?] failed to set anime index: {error}");
    }
}

/// 遍历番剧数据目录整体重建索引。
///
/// 先清空所有表再写入，删掉的番剧不会残留；这是 `build-index` 显式要求做的事，
/// 不该只打印一行警告就当作成功。
///
/// # Errors
/// 索引打不开、清空或写入失败时返回错误。
pub async fn build() -> Result<BuildResult, sqlx::Error> {
    let pool = POOL.as_ref().ok_or(sqlx::Error::PoolClosed)?;
    sqlx::query("DELETE FROM id_tag").execute(pool).await?;
    sqlx::query("DELETE FROM id_title").execute(pool).await?;
    sqlx::query("DELETE FROM anime").execute(pool).await?;

    let mut bangumi_ids = 0;
    let mut title_keys = 0;
    let mut tag_keys = 0;
    let mut errors = Vec::new();
    for anime in Anime::iter_all() {
        match insert_anime(pool, &anime).await {
            Ok((has_bangumi, titles, tags)) => {
                if has_bangumi {
                    bangumi_ids += 1;
                }
                title_keys += titles;
                tag_keys += tags;
            }
            Err(error) => errors.push(error),
        }
    }
    Ok(BuildResult {
        bangumi_id_to_meta_id: bangumi_ids,
        title_to_meta_id: title_keys,
        tag_to_meta_id: tag_keys,
        errors,
    })
}

/// 把一条番剧写进索引，返回 `(是否有 bangumi_id, 标题数, 标签数)`。
async fn insert_anime(
    pool: &SqlitePool,
    anime: &Anime,
) -> Result<(bool, usize, usize), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let anime_id = upsert_meta_id(&mut tx, anime.meta.id.id.as_str(), anime.bangumi_id).await?;
    let titles = rebuild_titles(&mut tx, anime_id, anime).await?;
    let tags = rebuild_tags(&mut tx, anime_id, anime).await?;
    tx.commit().await?;
    Ok((anime.bangumi_id.is_some(), titles, tags))
}

/// 插入/更新 `anime` 行，返回其自增 id。
///
/// 更新条目时 `meta.id` 可能被重算（`update-with-bangumi`），而 `bangumi_id` 是唯一的：
/// 先把指向同一 `bangumi_id` 的旧行删掉（其 id_title/id_tag 由外键级联清掉），再按
/// `meta_id` upsert，避免两个 `meta_id` 抢同一个 UNIQUE `bangumi_id`。
async fn upsert_meta_id(
    tx: &mut Transaction<'_, Sqlite>,
    meta_id: &str,
    bangumi_id: Option<i64>,
) -> Result<i64, sqlx::Error> {
    if let Some(bangumi_id) = bangumi_id {
        sqlx::query("DELETE FROM anime WHERE bangumi_id = ? AND meta_id <> ?")
            .bind(bangumi_id)
            .bind(meta_id)
            .execute(&mut **tx)
            .await?;
    }
    sqlx::query(
        "INSERT INTO anime (meta_id, bangumi_id) VALUES (?, ?)
         ON CONFLICT(meta_id) DO UPDATE SET bangumi_id = excluded.bangumi_id",
    )
    .bind(meta_id)
    .bind(bangumi_id)
    .execute(&mut **tx)
    .await?;
    let (id,): (i64,) = sqlx::query_as("SELECT id FROM anime WHERE meta_id = ?")
        .bind(meta_id)
        .fetch_one(&mut **tx)
        .await?;
    Ok(id)
}

/// 重建某条番剧的标题行（先删后插），返回写入的标题数。
async fn rebuild_titles(
    tx: &mut Transaction<'_, Sqlite>,
    anime_id: i64,
    anime: &Anime,
) -> Result<usize, sqlx::Error> {
    sqlx::query("DELETE FROM id_title WHERE id = ?")
        .bind(anime_id)
        .execute(&mut **tx)
        .await?;
    let mut count = 0;
    for (language, title) in title_pairs(anime) {
        sqlx::query("INSERT INTO id_title (id, language, title) VALUES (?, ?, ?)")
            .bind(anime_id)
            .bind(language)
            .bind(title)
            .execute(&mut **tx)
            .await?;
        count += 1;
    }
    Ok(count)
}

/// 重建某条番剧的标签行（先删后插），返回写入的标签数。
async fn rebuild_tags(
    tx: &mut Transaction<'_, Sqlite>,
    anime_id: i64,
    anime: &Anime,
) -> Result<usize, sqlx::Error> {
    sqlx::query("DELETE FROM id_tag WHERE id = ?")
        .bind(anime_id)
        .execute(&mut **tx)
        .await?;
    let mut seen = HashSet::new();
    let mut count = 0;
    for tag in &anime.info.tags {
        let normalized = tag.trim().to_lowercase();
        if normalized.is_empty() || !seen.insert(normalized.clone()) {
            continue;
        }
        sqlx::query("INSERT INTO id_tag (id, tag) VALUES (?, ?)")
            .bind(anime_id)
            .bind(&normalized)
            .execute(&mut **tx)
            .await?;
        count += 1;
    }
    Ok(count)
}

/// 这条番剧的全部 `(language, title)` 对，按标题去重（与 [`Anime::all_titles`] 一致）。
///
/// 原标题的语言按内容检测；各语言译名用它自己的 [`Language`]。空标题跳过。
fn title_pairs(anime: &Anime) -> Vec<(String, String)> {
    let mut seen = HashSet::new();
    let mut pairs = Vec::new();
    if !anime.info.title.trim().is_empty() && seen.insert(anime.info.title.clone()) {
        pairs.push((
            Language::detect_from(&anime.info.title).kind().to_string(),
            anime.info.title.clone(),
        ));
    }
    for (language, i18n) in &anime.internationalizations {
        if let Some(title) = i18n.title.as_deref()
            && !title.trim().is_empty()
            && seen.insert(title.to_string())
        {
            pairs.push((language.kind().to_string(), title.to_string()));
        }
    }
    pairs
}
