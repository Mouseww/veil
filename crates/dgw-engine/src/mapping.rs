use std::collections::HashMap;
use std::sync::Mutex;

use thiserror::Error;

use crate::creator::Creator;
use crate::placeholder::Placeholder;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("unavailable")]
    Unavailable,
    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lookup {
    Hit(String),
    Miss,
}

pub trait MappingStore {
    fn get_or_insert(
        &self,
        creator: &Creator,
        type_prefix: &str,
        plaintext: &str,
    ) -> Result<Placeholder, StoreError>;

    fn lookup(&self, creator: &Creator, placeholder: &Placeholder) -> Result<Lookup, StoreError>;
}

/// In-memory mapping store with interior mutability.
///
/// The same `(creator, plaintext)` pair reuses the placeholder from the first
/// insert, including that insert's `type_prefix`. Lookup requires a creator
/// match; a different creator is a [`Lookup::Miss`].
pub struct MemoryStore {
    inner: Mutex<MemoryStoreInner>,
}

#[derive(Default)]
struct MemoryStoreInner {
    by_plaintext: HashMap<(Creator, String), Placeholder>,
    by_token: HashMap<(Creator, String), String>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(MemoryStoreInner::default()),
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, MemoryStoreInner>, StoreError> {
        self.inner
            .lock()
            .map_err(|err| StoreError::Internal(err.to_string()))
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MappingStore for MemoryStore {
    fn get_or_insert(
        &self,
        creator: &Creator,
        type_prefix: &str,
        plaintext: &str,
    ) -> Result<Placeholder, StoreError> {
        let mut inner = self.lock()?;
        let key = (*creator, plaintext.to_string());
        if let Some(existing) = inner.by_plaintext.get(&key) {
            return Ok(existing.clone());
        }
        let placeholder = Placeholder::fresh(type_prefix);
        inner
            .by_token
            .insert((*creator, placeholder.format()), plaintext.to_string());
        inner.by_plaintext.insert(key, placeholder.clone());
        Ok(placeholder)
    }

    fn lookup(&self, creator: &Creator, placeholder: &Placeholder) -> Result<Lookup, StoreError> {
        let inner = self.lock()?;
        match inner.by_token.get(&(*creator, placeholder.format())) {
            Some(plaintext) => Ok(Lookup::Hit(plaintext.clone())),
            None => Ok(Lookup::Miss),
        }
    }
}

/// A [`MappingStore`] that always reports [`StoreError::Unavailable`].
pub struct DownStore;

impl MappingStore for DownStore {
    fn get_or_insert(
        &self,
        _creator: &Creator,
        _type_prefix: &str,
        _plaintext: &str,
    ) -> Result<Placeholder, StoreError> {
        Err(StoreError::Unavailable)
    }

    fn lookup(&self, _creator: &Creator, _placeholder: &Placeholder) -> Result<Lookup, StoreError> {
        Err(StoreError::Unavailable)
    }
}
