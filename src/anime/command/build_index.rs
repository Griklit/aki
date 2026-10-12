use super::super::index;

pub use super::super::index::BuildResult;

/// 重建番剧索引（`root/{anime.path}/.aki/anime-index.sqlite`，见
/// [`AnimeNamespace::sqlite_index_path()`](crate::anime::AnimeNamespace::sqlite_index_path)）。
///
/// 索引本身是可以随时从数据目录重建的缓存，因此这里不需要先清空整个数据库：
/// [`index::build`] 会先清空所有表再整体写入。
pub async fn build_index() -> Result<BuildResult, sqlx::Error> {
    index::build().await
}
