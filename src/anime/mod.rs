pub mod command;
mod context;
mod core;
pub mod index;
mod info;
mod namespace;
mod score;
mod state;

pub use context::{ANIME_CONTEXT, AnimeContext};
pub use core::{Anime, LoadAnimeError, SaveAnimeError};
pub use info::{AnimeInfo, AnimeInfoInternationalization};
pub use namespace::AnimeNamespace;
pub use score::AnimeScore;
pub use state::{AnimeState, AnimeWatchStatus};
