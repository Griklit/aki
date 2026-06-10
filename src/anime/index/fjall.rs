use std::ops::{Bound, RangeBounds};

pub struct FjallKeyspacewrapper<K, V> {
    pub keyspace: fjall::Keyspace,
    as_key: fn(&K) -> &[u8],
    as_value: fn(&V) -> &[u8],
    from_key: fn(&[u8]) -> K,
    from_value: fn(&[u8]) -> V,
}

impl<K, V> FjallKeyspacewrapper<K, V> {
    pub fn get(&self, key: &K) -> Result<Option<V>, fjall::Error> {
        let key = (self.as_key)(key);
        let value = self
            .keyspace
            .get(key)?
            .map(|v| (self.from_value)(v.as_ref()));
        Ok(value)
    }

    pub fn insert(&self, key: &K, value: &V) -> Result<(), fjall::Error> {
        let key = (self.as_key)(key);
        let value = (self.as_value)(value);
        self.keyspace.insert(key, value)
    }

    pub fn range(&self, range: impl RangeBounds<K>) -> fjall::Iter {
        struct MyTempCustomUnnamedRange<T> {
            start: Bound<T>,
            end: Bound<T>,
        }

        impl<T> RangeBounds<T> for MyTempCustomUnnamedRange<T> {
            fn start_bound(&self) -> Bound<&T> {
                self.start.as_ref()
            }
            fn end_bound(&self) -> Bound<&T> {
                self.end.as_ref()
            }
        }
        let range = MyTempCustomUnnamedRange {
            start: match range.start_bound() {
                Bound::Included(x) => Bound::Included((self.as_key)(x)),
                Bound::Excluded(x) => Bound::Excluded((self.as_key)(x)),
                Bound::Unbounded => Bound::Unbounded,
            },
            end: match range.end_bound() {
                Bound::Included(x) => Bound::Included((self.as_key)(x)),
                Bound::Excluded(x) => Bound::Excluded((self.as_key)(x)),
                Bound::Unbounded => Bound::Unbounded,
            },
        };
        self.keyspace.range(range)
    }
}

pub struct FjallIndex {
    db: fjall::Database,
    date: FjallKeyspacewrapper<u8, String>,
}
