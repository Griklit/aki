use crate::{CONFIG, Namespace, utils};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
pub struct GameNamespace;

impl Namespace for GameNamespace {
    fn new() -> Self {
        GameNamespace
    }

    fn kind() -> &'static str {
        "game"
    }

    fn data_path() -> PathBuf {
        utils::path::normalize(CONFIG.root.join(&CONFIG.game.path))
    }
}
