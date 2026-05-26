use crate::Score;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AnimeScore {
    pub positive: Option<f32>,      // 0.0~1.0
    pub negative: Option<f32>,      // 0.0~1.0
    pub recommendation: Option<u8>, // 1~5
}

impl AnimeScore {
    #[must_use]
    pub fn new(positive: Option<f32>, negative: Option<f32>, recommendation: Option<u8>) -> Self {
        Self {
            positive: positive.map(|v| v.clamp(0.0, 1.0)),
            negative: negative.map(|v| v.clamp(0.0, 1.0)),
            recommendation: recommendation.map(|v| v.clamp(1, 5)),
        }
    }
}

impl Score for AnimeScore {
    fn score(&self) -> f32 {
        if let (Some(positive), Some(negative)) = (self.positive, self.negative) {
            ((positive - negative) / 2.0 + 0.5).clamp(0.0, 1.0)
        } else {
            0.5
        }
    }
}

impl Default for AnimeScore {
    fn default() -> Self {
        Self::new(None, None, None)
    }
}
