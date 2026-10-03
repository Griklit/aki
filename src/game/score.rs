// use crate::Score;
// use serde::{Deserialize, Serialize};
//
// /// 项目自身对游戏的评分。
// 评分模型的字段还没定，先整个注释掉；`GameState` 里也还没有 `score` 字段。
// 复用番剧那套 `Score` trait（0.0~1.0）还是另起一套，定了再打开。
//
// #[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
// pub struct GameScore {
//     pub positive: Option<f32>,
//     pub negative: Option<f32>,
//     pub recommendation: Option<u8>,
// }
