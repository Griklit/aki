#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

pub mod anime;
mod common;
mod config;
pub mod game;
mod meta;
pub mod source;
pub mod tile_layout;
pub mod utils;

pub use common::{Language, Score};
pub use config::CONFIG;
pub use meta::{Meta, MetaData, MetaId, Namespace};
