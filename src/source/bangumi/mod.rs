pub mod command;
mod namespace;
mod season;
mod subject;

pub use namespace::BangumiNamespace;
pub use season::{Season, SeasonParseError};
pub use subject::{Subject, Type as SubjectType};
