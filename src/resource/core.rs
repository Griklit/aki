use super::resource_type;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Resource<T: resource_type::ResourceType> {
    pub id: uuid::Uuid,
    pub info: T,
}

impl<T: resource_type::ResourceType> Resource<T> {}
