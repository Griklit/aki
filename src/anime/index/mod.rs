mod sqlite;

pub use sqlite::{
    BuildResult, build, get_meta_id_of_bangumi_id, has_tag_entries, is_available, meta_ids_of_tag,
    set_anime_index_or_warn, title_entries,
};
