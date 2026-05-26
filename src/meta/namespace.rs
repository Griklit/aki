use serde::Deserialize;

use serde::{Deserializer, Serializer};

pub(super) fn serialize_namespace<T, S>(_: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: Namespace,
    S: Serializer,
{
    serializer.serialize_str(T::kind())
}

pub(super) fn deserialize_namespace<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Namespace,
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let namespace_kind = T::kind();
    if s != namespace_kind {
        return Err(serde::de::Error::custom(format!(
            "namespace mismatch: {namespace_kind} required, but {s} found",
        )));
    }
    Ok(T::new())
}

pub trait Namespace {
    fn new() -> Self;
    fn kind() -> &'static str;
}
