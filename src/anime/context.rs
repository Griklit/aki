use super::index;
use std::sync::{LazyLock, OnceLock};
use tantivy::IndexReader;

pub static ANIME_CONTEXT: LazyLock<AnimeContext> = LazyLock::new(AnimeContext::new);

#[derive(Debug, thiserror::Error)]
pub enum InitTantivyReaderError {
    #[error("get tantivy index failed")]
    GetIndex(#[source] index::GetTantivyIndexError),
    #[error("build tantivy reader failed")]
    BuildReader(#[source] tantivy::TantivyError),
}

pub struct AnimeContext {
    pub tantivy_reader: OnceLock<IndexReader>,
    pub tantivy_schema: OnceLock<index::AnimeTantivyIndexSchema>,
}

impl AnimeContext {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tantivy_reader: OnceLock::new(),
            tantivy_schema: OnceLock::new(),
        }
    }

    pub fn init_tantivy_reader(&self) -> Result<(), InitTantivyReaderError> {
        type E = InitTantivyReaderError;
        let _ = self.tantivy_reader.set(
            index::get_tantivy_index()
                .map_err(E::GetIndex)?
                .reader_builder()
                .reload_policy(tantivy::ReloadPolicy::OnCommitWithDelay)
                .try_into()
                .map_err(E::BuildReader)?,
        );
        Ok(())
    }

    /// # Panics
    /// If the tantivy reader initialize failed.
    #[must_use]
    pub fn tantivy_reader(&self) -> &IndexReader {
        self.tantivy_reader.get_or_init(|| {
            index::get_tantivy_index()
                .expect("init tantivy index failed.")
                .reader_builder()
                .reload_policy(tantivy::ReloadPolicy::OnCommitWithDelay)
                .try_into()
                .expect("init tantivy reader failed.")
        })
    }

    #[must_use]
    pub fn tantivy_schema(&self) -> &index::AnimeTantivyIndexSchema {
        self.tantivy_schema
            .get_or_init(index::AnimeTantivyIndexSchema::new)
    }
}

impl Default for AnimeContext {
    fn default() -> Self {
        Self::new()
    }
}
