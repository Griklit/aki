use super::super::index;

pub use super::super::index::BuildResult;

/// 重建番剧索引（`root/{anime.path}/.aki/anime-index.sqlite`）。
///
/// 索引是可随时从数据目录重建的缓存，[`index::build`] 会先清空所有表再整体写入。
pub async fn build_index() -> Result<BuildResult, sqlx::Error> {
    index::build().await
}
