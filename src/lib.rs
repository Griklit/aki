#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

mod anime;
mod bangumi;
mod common;
mod config;
mod meta;
mod resource;
pub mod tile_layout;
pub mod utils;

pub use anime::{
    Anime, AnimeContext, AnimeInfo, AnimeInfoInternationalization, AnimeScore, AnimeState,
    AnimeWatchStatus, LoadAnimeError, SaveAnimeError, command as anime_command,
    index as anime_index,
};
pub use bangumi::{Subject as BangumiSubject, SubjectType as BangumiSubjectType};
pub use common::{Language, Score};
pub use config::CONFIG;
pub use meta::{Meta, MetaData, MetaId, Namespace};
pub use resource::{ImageType, Resource, cover};
