mod sqlite;

pub use sqlite::{
    BuildResult, build, get_meta_id_of_bangumi_id, has_tag_entries, has_title_entries,
    is_available, meta_ids_of_tag, meta_ids_of_title, set_anime_index_or_warn,
};
