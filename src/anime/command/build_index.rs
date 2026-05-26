use super::super::{ANIME_CONTEXT, index};

#[derive(Debug, thiserror::Error)]
pub enum BuildIndexErropr {
    #[error("build tantivy index error")]
    Tantivy(#[source] index::BuildTantivyIndexError),
}

pub fn build_index() -> Result<usize, BuildIndexErropr> {
    index::build_tantivy_index(&ANIME_CONTEXT).map_err(BuildIndexErropr::Tantivy)
}
