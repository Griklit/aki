use super::super::index;

pub use super::super::index::BuildResult;

/// 重建番剧索引（`{anime_data_path}/.aki/fjall-index/`）。
///
/// 索引本身是可以随时从数据目录重建的缓存，因此这里不需要先清空整个数据库：
/// [`index::Index::build`] 会先清空两个 keyspace 再整体写入。
pub fn build_index() -> Result<BuildResult, fjall::Error> {
    index::AnimeIndex::build()
}
