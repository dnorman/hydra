use anyhow::anyhow;
use sled::IVec;
use ulid::Ulid;

use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum Order {
    Ascending,
    Descending,
}
pub trait Key {
    type Bytes: AsRef<[u8]>;
    fn as_bytes(&self) -> Self::Bytes;
}

impl Key for Vec<u8> {
    type Bytes = Self;
    fn as_bytes(&self) -> Self::Bytes {
        // hack
        self.to_vec()
    }
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

pub struct FetchQuery<K: Key> {
    cursor: Option<K>,
    limit: Option<usize>,
    order: Order,
}

impl<K: Key> FetchQuery<K> {
    pub fn new() -> Self {
        FetchQuery {
            cursor: None,
            limit: None,
            order: Order::Ascending,
        }
    }

    pub fn cursor(mut self, value: K) -> Self {
        self.cursor = Some(value);
        self
    }

    pub fn limit(mut self, value: usize) -> Self {
        self.limit = Some(value);
        self
    }

    pub fn order(mut self, order: Order) -> Self {
        self.order = order;
        self
    }
}

pub trait Record: serde::de::DeserializeOwned {
    type ID: Clone;
    fn id(&self) -> &Self::ID;
}

pub struct FetchResult<T: Record> {
    pub items: Vec<(IVec, T)>,
    pub order: Order,
    pub more_records: bool,
}

impl<T: Record> FetchResult<T> {
    pub fn ids(&self) -> Vec<T::ID> {
        self.items.iter().map(|(_, r)| r.id().clone()).collect()
    }
}

use std::ops::Bound;

pub fn fetch<T: Record, K: Key>(
    tree: &sled::Tree,
    query: FetchQuery<K>,
) -> Result<FetchResult<T>, AppError> {
    let limit = query.limit.unwrap_or(10);
    let fetch_limit = limit + 1; // Fetch one extra to determine if there are more records

    let mut items = Vec::with_capacity(fetch_limit);

    match query.order {
        Order::Ascending => {
            let iter = match query.cursor {
                Some(cursor) => tree.range((Bound::Excluded(cursor.as_bytes()), Bound::Unbounded)),
                None => tree.iter(),
            };
            for item in iter.take(fetch_limit) {
                let (key, value) = item?;
                items.push((key, bincode::deserialize(&value)?));
            }
        }
        Order::Descending => {
            let iter = match query.cursor {
                Some(cursor) => tree
                    .range((Bound::Unbounded, Bound::Excluded(cursor.as_bytes())))
                    .rev(),
                None => tree.iter().rev(),
            };
            for item in iter.take(fetch_limit) {
                let (key, value) = item?;
                items.push((key, bincode::deserialize(&value)?));
            }
        }
    }

    let more_records = items.len() > limit;
    items.truncate(limit);

    Ok(FetchResult {
        items,
        more_records,
        order: query.order,
    })
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
        for id in 0usize..12 {
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

        // "next page" button is shown
        assert!(result.more_records);

        // user clicks "next page"
        let query = FetchQuery::<usize>::new().cursor(4).limit(5);
        let result = fetch::<TestRecord, _>(&tree, query).unwrap();
        assert_eq!(result.items.len(), 5);
        assert_eq!(result.ids(), &[5, 6, 7, 8, 9]);

        // "next page" button is shown
        assert!(result.more_records);

        // user clicks "next page" and a partial page is returned
        let query = FetchQuery::<usize>::new().cursor(9).limit(5);
        let result = fetch::<TestRecord, _>(&tree, query).unwrap();
        assert_eq!(result.items.len(), 2);
        assert_eq!(result.ids(), &[10, 11]);

        // "next page" button is not shown
        assert!(!result.more_records);

        // user clicks "previous page" button
        let query = FetchQuery::<usize>::new()
            .cursor(10)
            .limit(5)
            .order(Order::Descending);
        let result = fetch::<TestRecord, _>(&tree, query).unwrap();
        assert_eq!(result.items.len(), 5);
        assert_eq!(result.ids(), &[9, 8, 7, 6, 5]);
        // "previous page" button is shown
        assert!(result.more_records);

        // user clicks "previous page" button
        let query = FetchQuery::<usize>::new()
            .cursor(5)
            .limit(5)
            .order(Order::Descending);
        let result = fetch::<TestRecord, _>(&tree, query).unwrap();
        assert_eq!(result.items.len(), 5);
        assert_eq!(result.ids(), &[4, 3, 2, 1, 0]);

        // "previous page" button is not shown
        assert!(!result.more_records);

        // lets test the case where the cursor is the first record
        let query = FetchQuery::<usize>::new().cursor(0).limit(5);
        let result = fetch::<TestRecord, _>(&tree, query).unwrap();
        assert_eq!(result.items.len(), 5);
        assert_eq!(result.ids(), &[1, 2, 3, 4, 5]);
        assert!(result.more_records);

        // now lets check what happens when the cursor is the first record and we're descending
        let query = FetchQuery::<usize>::new()
            .cursor(0)
            .limit(5)
            .order(Order::Descending);
        let result = fetch::<TestRecord, _>(&tree, query).unwrap();
        assert_eq!(result.items.len(), 0);
        assert!(!result.more_records);

        // Lets do last cursor ascending
        let query = FetchQuery::<usize>::new()
            .cursor(11)
            .limit(5)
            .order(Order::Ascending);
        let result = fetch::<TestRecord, _>(&tree, query).unwrap();
        assert_eq!(result.items.len(), 0);
        assert!(!result.more_records);
    }
}
