use std::borrow::Borrow;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

/// Small fixed-capacity LRU cache for process-wide compiled resources.
///
/// Eviction is deliberately lossless: callers rebuild a missing value, so a
/// capacity limit can only change the hit rate, never observable results.
pub(crate) struct LruCache<K, V> {
    entries: HashMap<K, V>,
    order: VecDeque<K>,
    capacity: usize,
}

impl<K, V> LruCache<K, V>
where
    K: Clone + Eq + Hash,
{
    pub(crate) fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "LRU cache capacity must be greater than zero");
        Self {
            entries: HashMap::with_capacity(capacity),
            order: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub(crate) fn get<Q>(&mut self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        if !self.entries.contains_key(key) {
            return None;
        }
        self.promote(key);
        self.entries.get(key)
    }

    pub(crate) fn insert(&mut self, key: K, value: V) {
        if self.entries.contains_key(&key) {
            self.entries.insert(key.clone(), value);
            self.promote(&key);
            return;
        }

        if self.entries.len() == self.capacity {
            let oldest = self.order.pop_front().expect("LRU order is not empty");
            self.entries.remove(&oldest);
        }
        self.order.push_back(key.clone());
        self.entries.insert(key, value);
    }

    fn promote<Q>(&mut self, key: &Q)
    where
        K: Borrow<Q>,
        Q: Eq + ?Sized,
    {
        if self
            .order
            .back()
            .is_some_and(|candidate| candidate.borrow() == key)
        {
            return;
        }
        let index = self
            .order
            .iter()
            .rposition(|candidate| candidate.borrow() == key)
            .expect("cached key is present in LRU order");
        let key = self.order.remove(index).expect("LRU index is valid");
        self.order.push_back(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_and_reuse_follow_lru_order() {
        let mut cache = LruCache::new(2);
        cache.insert("first", 1);
        cache.insert("second", 2);

        assert_eq!(cache.get("first"), Some(&1));

        cache.insert("third", 3);
        assert_eq!(cache.entries.len(), 2);
        assert_eq!(cache.get("first"), Some(&1));
        assert_eq!(cache.get("second"), None);
        assert_eq!(cache.get("third"), Some(&3));
    }

    #[test]
    fn replacing_an_entry_does_not_consume_capacity() {
        let mut cache = LruCache::new(2);
        cache.insert("first", 1);
        cache.insert("second", 2);
        cache.insert("first", 10);
        cache.insert("third", 3);

        assert_eq!(cache.entries.len(), 2);
        assert_eq!(cache.get("first"), Some(&10));
        assert_eq!(cache.get("second"), None);
        assert_eq!(cache.get("third"), Some(&3));
    }

    #[test]
    fn reading_the_hottest_entry_keeps_the_order_unchanged() {
        let mut cache = LruCache::new(2);
        cache.insert("first", 1);
        cache.insert("second", 2);

        assert_eq!(cache.get("second"), Some(&2));
        assert_eq!(cache.order, VecDeque::from(["first", "second"]));
    }
}
