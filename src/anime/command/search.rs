use super::super::{ANIME_CONTEXT, Anime, index};

pub fn search(query: &str, limit: usize) -> Result<Vec<Anime>, index::SearchTantivyError> {
    index::search_tantivy(&ANIME_CONTEXT, query, limit)
}
