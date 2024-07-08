use anyhow::anyhow;
use sled::IVec;
use ulid::Ulid;

use crate::error::AppError;

pub enum Order {
    Ascending,
    Descending,
}
pub trait Key {
    type Bytes: AsRef<[u8]>;
    fn as_bytes(&self) -> Self::Bytes;
}

impl Key for Ulid {
    type Bytes = &'static [u8; 16];
    fn as_bytes(&self) -> Self::Bytes {
        self.as_bytes()
    }
}

impl Key for usize {
    type Bytes = [u8; std::mem::size_of::<usize>()];
    fn as_bytes(&self) -> Self::Bytes {
        self.to_be_bytes()
    }
}

pub struct FetchQuery<ID: Key> {
    earlier_than: Option<ID>,
    later_than: Option<ID>,
    limit: Option<usize>,
    order: Order,
}

impl<K: Key> FetchQuery<K> {
    pub fn new() -> Self {
        FetchQuery {
            earlier_than: None,
            later_than: None,
            limit: None,
            order: Order::Ascending,
        }
    }

    pub fn earlier_than(mut self, value: K) -> Self {
        self.earlier_than = Some(value);
        self
    }

    pub fn later_than(mut self, value: K) -> Self {
        self.later_than = Some(value);
        self
    }

    pub fn limit(mut self, value: usize) -> Self {
        self.limit = Some(value);
        self
    }
}

pub trait Record: serde::de::DeserializeOwned {
    type ID: Clone;
    fn id(&self) -> &Self::ID;
}

pub struct FetchResult<T: Record> {
    pub items: Vec<(IVec, T)>,
    pub earlier_records_present: bool,
    pub later_records_present: bool,
}

impl<T: Record> FetchResult<T> {
    pub fn ids(&self) -> Vec<T::ID> {
        self.items.iter().map(|(_, r)| r.id().clone()).collect()
    }
}

pub fn fetch<T: Record, K: Key>(
    tree: &sled::Tree,
    query: FetchQuery<K>,
) -> Result<FetchResult<T>, AppError> {
    let limit = query.limit.unwrap_or(10);
    let fetch_limit = limit + 1; // Fetch one extra to determine if there's a next page

    match (query.earlier_than, query.later_than) {
        (Some(end), None) => {
            let mut vec: Vec<_> = tree
                .range(..end.as_bytes())
                .rev()
                .take(fetch_limit)
                .map(|item| {
                    let (key, value) = item.unwrap();
                    (key, bincode::deserialize(&value).unwrap())
                })
                .collect();
            let has_earlier = vec.len() > limit;
            vec.truncate(limit);
            vec.reverse();
            Ok(FetchResult {
                items: vec,
                earlier_records_present: has_earlier,
                later_records_present: true,
            })
        }
        (None, Some(start)) => {
            let mut vec: Vec<_> = tree
                .range(start.as_bytes()..)
                .take(fetch_limit)
                .map(|item| {
                    let (key, value) = item.unwrap();
                    (key, bincode::deserialize(&value).unwrap())
                })
                .collect();

            let has_later = vec.len() > limit;
            vec.truncate(limit);

            Ok(FetchResult {
                items: vec,
                earlier_records_present: false,
                later_records_present: has_later,
            })
        }
        (None, None) => {
            let mut vec: Vec<_> = tree
                .iter()
                .rev()
                .take(fetch_limit)
                .map(|item| {
                    let (key, value) = item.unwrap();
                    (key, bincode::deserialize(&value).unwrap())
                })
                .collect();
            let has_earlier = vec.len() > limit;
            vec.truncate(limit);
            Ok(FetchResult {
                items: vec,
                earlier_records_present: has_earlier,
                later_records_present: false,
            })
        }
        _ => Err(anyhow!("Cannot specify both earlier_than and later_than").into()),
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::storage::StorageEngine;

    #[derive(Serialize, Deserialize)]
    pub struct TestRecord {
        pub id: usize,
        pub value: String,
    }

    impl Record for TestRecord {
        type ID = usize;
        fn id(&self) -> &Self::ID {
            &self.id
        }
    }

    #[test]
    fn test_fetch() {
        let storage = StorageEngine::new_test().unwrap();

        let tree = storage.subtree("test").unwrap();

        // first we have to load up the db with test records
        for id in 0usize..100 {
            let record = TestRecord {
                id,
                value: format!("test value {}", id),
            };

            // use BigEndian to ensure lexicographic ordering
            tree.insert(&id.to_be_bytes(), bincode::serialize(&record).unwrap())
                .unwrap();
        }

        // now lets run fetch with
        let query = FetchQuery::<usize>::new().limit(5);
        let result = fetch::<TestRecord, _>(&tree, query).unwrap();

        // the default is ascending, so the first 5 should be the oldest 5
        assert_eq!(result.items.len(), 5);
        assert_eq!(result.ids(), &[0, 1, 2, 3, 4]);
        assert!(!result.earlier_records_present);
        assert!(result.later_records_present);

        // next page
        let query = FetchQuery::<usize>::new().later_than(4).limit(5);
        let result = fetch::<TestRecord, _>(&tree, query).unwrap();
        assert_eq!(result.items.len(), 5);
        assert_eq!(result.ids(), &[5, 6, 7, 8, 9]);
        assert!(result.earlier_records_present);
        assert!(!result.later_records_present);
    }
}
