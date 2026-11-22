//! Collection types for ArceOS applications.

pub use alloc::collections::*;

use alloc::vec::Vec;
use core::borrow::Borrow;
use core::hash::{Hash, Hasher};

/// A hash table with separate chaining and a deterministic hash function.
pub struct HashMap<K, V> {
    buckets: Vec<Vec<(K, V)>>,
    len: usize,
}

impl<K, V> HashMap<K, V> {
    /// Creates an empty map without allocating buckets.
    pub const fn new() -> Self {
        Self {
            buckets: Vec::new(),
            len: 0,
        }
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns whether the map contains no entries.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Visits the map's entries in an arbitrary order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.buckets
            .iter()
            .flatten()
            .map(|(key, value)| (key, value))
    }
}

impl<K, V> Default for HashMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Eq + Hash, V> HashMap<K, V> {
    /// Adds an entry and returns the previous value, if any.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if let Some(old) = self.get_mut(&key) {
            return Some(core::mem::replace(old, value));
        }
        if self.len == self.buckets.len() {
            self.grow();
        }
        let index = bucket_index(&key, self.buckets.len());
        self.buckets[index].push((key, value));
        self.len += 1;
        None
    }

    /// Returns a reference to the value corresponding to a key.
    pub fn get<Q: Eq + Hash + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        let (bucket, entry) = self.find(key)?;
        Some(&self.buckets[bucket][entry].1)
    }

    /// Returns a mutable reference to the value corresponding to a key.
    pub fn get_mut<Q: Eq + Hash + ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
    {
        let (bucket, entry) = self.find(key)?;
        Some(&mut self.buckets[bucket][entry].1)
    }

    /// Removes a key and returns its value, if present.
    pub fn remove<Q: Eq + Hash + ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
    {
        let (bucket, entry) = self.find(key)?;
        self.len -= 1;
        Some(self.buckets[bucket].swap_remove(entry).1)
    }

    fn find<Q: Eq + Hash + ?Sized>(&self, key: &Q) -> Option<(usize, usize)>
    where
        K: Borrow<Q>,
    {
        if self.is_empty() {
            return None;
        }
        let index = bucket_index(key, self.buckets.len());
        let entry = self.buckets[index]
            .iter()
            .position(|(stored, _)| stored.borrow() == key)?;
        Some((index, entry))
    }

    fn grow(&mut self) {
        let new_size = self.buckets.len().max(4) * 2;
        let mut buckets = Vec::with_capacity(new_size);
        buckets.resize_with(new_size, Vec::new);
        for (key, value) in core::mem::take(&mut self.buckets).into_iter().flatten() {
            buckets[bucket_index(&key, new_size)].push((key, value));
        }
        self.buckets = buckets;
    }
}

fn bucket_index(key: &(impl Hash + ?Sized), bucket_count: usize) -> usize {
    let mut hasher = FnvHasher(0xcbf29ce484222325);
    key.hash(&mut hasher);
    (hasher.finish() as usize) & (bucket_count - 1)
}

struct FnvHasher(u64);

impl Hasher for FnvHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
}
