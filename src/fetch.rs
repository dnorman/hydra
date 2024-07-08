use anyhow::anyhow;
use base64::{engine::general_purpose::URL_SAFE, Engine as _};
use sled::IVec;

use crate::error::AppError;

pub trait AsBytes {
    fn as_bytes(&self) -> &[u8];
}

impl<T: AsRef<[u8]>> AsBytes for T {
    fn as_bytes(&self) -> &[u8] {
        self.as_ref()
    }
}

pub struct FetchQuery<T: AsBytes> {
    earlier_than: Option<T>,
    later_than: Option<T>,
    limit: Option<usize>,
}

pub struct FetchResult<T> {
    pub items: Vec<(IVec, T)>,
    pub has_earlier_page: bool,
    pub has_later_page: bool,
}

pub fn fetch<T: serde::de::DeserializeOwned, B: AsBytes>(
    tree: &sled::Tree,
    query: FetchQuery<B>,
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
                has_earlier_page: has_earlier,
                has_later_page: true,
            })
        }
        (None, Some(start)) => {
            let vec: Vec<_> = tree
                .range(start.as_bytes()..)
                .take(fetch_limit)
                .map(|item| {
                    let (key, value) = item.unwrap();
                    (key, bincode::deserialize(&value).unwrap())
                })
                .collect();

            let has_later = vec.len() > limit;

            Ok(FetchResult {
                items: vec[..limit.min(vec.len())].to_vec(),
                has_earlier_page: false,
                has_later_page: has_later,
            })
        }
        (None, None) => {
            let vec: Vec<_> = tree
                .iter()
                .rev()
                .take(fetch_limit)
                .map(|item| {
                    let (key, value) = item.unwrap();
                    (key, bincode::deserialize(&value).unwrap())
                })
                .collect();
            let has_earlier = vec.len() > limit;
            Ok(FetchResult {
                items: vec[..limit.min(vec.len())].to_vec(),
                has_earlier_page: has_earlier,
                has_later_page: false,
            })
        }
        _ => Err(anyhow!("Cannot specify both earlier_than and later_than").into()),
    }
}

pub fn decode_url_safe(input: &str) -> Result<Vec<u8>, AppError> {
    URL_SAFE.decode(input).map_err(|e| anyhow!(e).into())
}

#[cfg(test)]
mod tests {}
