use std::hash::Hash;

use rustc_hash::FxHashMap;

#[derive(Debug)]
pub(super) struct AdaptiveLruCache<K, V> {
    entries: FxHashMap<K, CacheEntry<V>>,
    tick: u64,
    base_limit: usize,
    max_limit: usize,
    current_limit: usize,
    pressure_count: u8,
    slack_count: u64,
}

#[derive(Debug)]
struct CacheEntry<V> {
    value: V,
    last_used: u64,
}

impl<K, V> AdaptiveLruCache<K, V>
where
    K: Clone + Eq + Hash,
{
    pub(super) fn new(base_limit: usize, max_limit: usize) -> Self {
        let base_limit = base_limit.max(1);
        let max_limit = max_limit.max(base_limit);
        Self {
            entries: FxHashMap::default(),
            tick: 0,
            base_limit,
            max_limit,
            current_limit: base_limit,
            pressure_count: 0,
            slack_count: 0,
        }
    }

    pub(super) fn get(&mut self, key: &K) -> Option<&V> {
        self.observe_utilization();
        let tick = self.next_tick();
        let entry = self.entries.get_mut(key)?;
        entry.last_used = tick;
        Some(&entry.value)
    }

    pub(super) fn insert(&mut self, key: K, value: V) {
        let tick = self.next_tick();
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.value = value;
            entry.last_used = tick;
            self.observe_utilization();
            return;
        }

        self.reserve_slot();
        self.entries.insert(
            key,
            CacheEntry {
                value,
                last_used: tick,
            },
        );
        self.observe_utilization();
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.tick = 0;
        self.current_limit = self.base_limit;
        self.pressure_count = 0;
        self.slack_count = 0;
    }

    fn next_tick(&mut self) -> u64 {
        self.tick = self.tick.saturating_add(1);
        self.tick
    }

    fn reserve_slot(&mut self) {
        if self.entries.len() < self.current_limit {
            return;
        }

        self.evict_lru_entries(1);
        self.pressure_count = self.pressure_count.saturating_add(1);
        self.slack_count = 0;
        if self.pressure_count >= 3 && self.current_limit < self.max_limit {
            let grown = self
                .current_limit
                .saturating_add((self.current_limit / 2).max(1));
            self.current_limit = grown.min(self.max_limit);
            self.pressure_count = 0;
        }
    }

    fn evict_lru_entries(&mut self, count: usize) {
        if count == 0 || self.entries.is_empty() {
            return;
        }

        let mut keys = self
            .entries
            .iter()
            .map(|(key, entry)| (key.clone(), entry.last_used))
            .collect::<Vec<_>>();
        keys.sort_unstable_by_key(|(_, last_used)| *last_used);

        for (key, _) in keys.into_iter().take(count) {
            self.entries.remove(&key);
        }
    }

    fn observe_utilization(&mut self) {
        if self.current_limit <= self.base_limit {
            return;
        }

        if self.entries.len().saturating_mul(2) > self.current_limit {
            self.slack_count = 0;
            return;
        }

        self.slack_count = self.slack_count.saturating_add(1);
        if self.slack_count < self.current_limit as u64 {
            return;
        }

        let desired_limit = self
            .entries
            .len()
            .max(1)
            .saturating_mul(2)
            .next_power_of_two()
            .max(self.base_limit);
        self.current_limit = desired_limit.min(self.current_limit);
        self.pressure_count = 0;
        self.slack_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::AdaptiveLruCache;

    impl<K, V> AdaptiveLruCache<K, V> {
        fn current_limit_for_test(&self) -> usize {
            self.current_limit
        }
    }

    #[test]
    fn evicts_least_recently_used_entry_at_capacity() {
        let mut cache = AdaptiveLruCache::new(2, 4);
        cache.insert("a", 1);
        cache.insert("b", 2);
        let _ = cache.get(&"a");
        cache.insert("c", 3);

        assert_eq!(cache.get(&"a"), Some(&1));
        assert_eq!(cache.get(&"c"), Some(&3));
        assert_eq!(cache.get(&"b"), None);
    }

    #[test]
    fn grows_after_repeated_pressure() {
        let mut cache = AdaptiveLruCache::new(2, 8);
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.insert("c", 3);
        cache.insert("d", 4);
        cache.insert("e", 5);

        assert!(cache.current_limit_for_test() > 2);
    }

    #[test]
    fn shrinks_after_long_low_utilization_period() {
        let mut cache = AdaptiveLruCache::new(2, 8);
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.insert("c", 3);
        cache.insert("d", 4);
        cache.insert("e", 5);
        let grown_limit = cache.current_limit_for_test();
        assert!(grown_limit > 2);

        cache.entries.remove(&"a");
        cache.entries.remove(&"b");
        cache.entries.remove(&"c");
        cache.entries.remove(&"d");

        for _ in 0..grown_limit {
            cache.insert("e", 5);
        }

        assert!(cache.current_limit_for_test() < grown_limit);
        assert!(cache.current_limit_for_test() >= 2);
    }
}
