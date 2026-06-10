// mod fjall;
mod tantivy;

pub use tantivy::{
    AnimeTantivyIndexSchema, BuildTantivyIndexError, GetTantivyIndexError, SearchTantivyError,
    build_tantivy_index, get_tantivy_index, search_tantivy,
};
