use crate::domain::{DiagramEngineType, GraphicsProtocol, ResourceLimits, ThemeMode};
use std::collections::VecDeque;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

/// Clé composite identifiant de manière unique un diagramme et son contexte de rendu.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RenderCacheKey {
    pub content: String,
    pub theme: ThemeMode,
    pub width: u16,
    pub protocol: GraphicsProtocol,
    pub engine: DiagramEngineType,
}

impl RenderCacheKey {
    #[must_use]
    pub fn new(
        content: &str,
        theme: ThemeMode,
        width: u16,
        protocol: GraphicsProtocol,
        engine: DiagramEngineType,
    ) -> Self {
        Self {
            content: content.to_string(),
            theme,
            width,
            protocol,
            engine,
        }
    }
}

/// Bornes du cache : nombre d'entrées et octets cumulés (source + payload).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheBudget {
    pub max_entries: usize,
    pub max_bytes: usize,
}

impl From<&ResourceLimits> for CacheBudget {
    fn from(limits: &ResourceLimits) -> Self {
        Self {
            max_entries: limits.max_cached_diagrams,
            max_bytes: limits.max_cache_bytes,
        }
    }
}

#[derive(Debug)]
struct CacheEntry {
    key: RenderCacheKey,
    payload: String,
    size: usize,
}

#[derive(Debug, Default)]
struct CacheState {
    entries: VecDeque<CacheEntry>,
    bytes: usize,
}

impl CacheState {
    fn take(&mut self, key: &RenderCacheKey) -> Option<CacheEntry> {
        let idx = self.entries.iter().position(|entry| &entry.key == key)?;
        let entry = self.entries.remove(idx)?;
        self.bytes -= entry.size;
        Some(entry)
    }

    fn push_front(&mut self, entry: CacheEntry) {
        self.bytes += entry.size;
        self.entries.push_front(entry);
    }

    fn evict_to(&mut self, budget: CacheBudget) {
        while self.entries.len() > budget.max_entries || self.bytes > budget.max_bytes {
            let Some(evicted) = self.entries.pop_back() else {
                return;
            };
            self.bytes -= evicted.size;
        }
    }
}

/// Cache LRU in-process thread-safe pour les rendus terminaux de diagrammes,
/// borné en nombre d'entrées et en octets.
#[derive(Debug)]
pub struct RenderCache {
    state: Mutex<CacheState>,
    budget: CacheBudget,
}

static GLOBAL_CACHE: LazyLock<RenderCache> =
    LazyLock::new(|| RenderCache::new(CacheBudget::from(&ResourceLimits::default())));

impl RenderCache {
    #[must_use]
    pub fn new(budget: CacheBudget) -> Self {
        Self {
            state: Mutex::new(CacheState::default()),
            budget,
        }
    }

    /// Référence vers le cache global partagé de l'application.
    #[must_use]
    pub fn global() -> &'static Self {
        &GLOBAL_CACHE
    }

    /// Recherche un rendu dans le cache et le replace en tête (LRU).
    #[must_use]
    pub fn get(&self, key: &RenderCacheKey) -> Option<String> {
        let mut state = self.lock();
        let entry = state.take(key)?;
        let payload = entry.payload.clone();
        state.push_front(entry);
        Some(payload)
    }

    /// Insère un rendu puis évince les entrées les moins récentes jusqu'à respecter le budget.
    /// Une entrée excédant à elle seule le budget en octets n'est pas cachée.
    pub fn insert(&self, key: RenderCacheKey, payload: String) {
        let size = key.content.len() + payload.len();
        if self.budget.max_entries == 0 || size > self.budget.max_bytes {
            return;
        }

        let mut state = self.lock();
        let _replaced = state.take(&key);
        state.push_front(CacheEntry { key, payload, size });
        state.evict_to(self.budget);
    }

    /// Vide l'intégralité du cache.
    pub fn clear(&self) {
        *self.lock() = CacheState::default();
    }

    /// Nombre actuel d'entrées en cache.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().entries.len()
    }

    /// Indique si le cache est vide.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Octets cumulés (source + payload) des entrées en cache.
    #[must_use]
    pub fn size_bytes(&self) -> usize {
        self.lock().bytes
    }

    /// Les sections critiques ne paniquent pas : un verrou empoisonné conserve un état cohérent.
    fn lock(&self) -> MutexGuard<'_, CacheState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(content: &str) -> RenderCacheKey {
        RenderCacheKey::new(
            content,
            ThemeMode::Dark,
            80,
            GraphicsProtocol::HalfBlocks,
            DiagramEngineType::default(),
        )
    }

    fn budget(max_entries: usize, max_bytes: usize) -> CacheBudget {
        CacheBudget {
            max_entries,
            max_bytes,
        }
    }

    #[test]
    fn test_cache_insert_and_get() {
        let cache = RenderCache::new(budget(2, 1024));
        let key1 = key("graph TD\nA-->B");
        let key2 = RenderCacheKey::new(
            "graph LR\nA-->B",
            ThemeMode::Light,
            80,
            GraphicsProtocol::Kitty,
            DiagramEngineType::default(),
        );

        cache.insert(key1.clone(), "render1".to_string());
        cache.insert(key2.clone(), "render2".to_string());

        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(&key1), Some("render1".to_string()));
        assert_eq!(cache.get(&key2), Some("render2".to_string()));
    }

    #[test]
    fn test_cache_lru_eviction() {
        let cache = RenderCache::new(budget(2, 1024));
        let (key1, key2, key3) = (key("k1"), key("k2"), key("k3"));

        cache.insert(key1.clone(), "r1".to_string());
        cache.insert(key2.clone(), "r2".to_string());

        // Accès à key1 pour rafraîchir sa position LRU
        let _ = cache.get(&key1);

        // Insertion de key3: doit évincer key2
        cache.insert(key3.clone(), "r3".to_string());

        assert_eq!(cache.get(&key1), Some("r1".to_string()));
        assert_eq!(cache.get(&key2), None);
        assert_eq!(cache.get(&key3), Some("r3".to_string()));
    }

    #[test]
    fn test_cache_zero_capacity() {
        let cache = RenderCache::new(budget(0, 1024));
        let key = key("k");
        cache.insert(key.clone(), "r".to_string());
        assert!(cache.is_empty());
        assert_eq!(cache.get(&key), None);
    }

    #[test]
    fn test_cache_evicts_least_recent_entries_by_bytes() {
        let cache = RenderCache::new(budget(10, 25));
        let (key1, key2, key3) = (key("k1"), key("k2"), key("k3"));

        cache.insert(key1.clone(), "x".repeat(8));
        cache.insert(key2.clone(), "y".repeat(8));
        cache.insert(key3.clone(), "z".repeat(8));

        assert_eq!(cache.get(&key1), None);
        assert!(cache.get(&key2).is_some());
        assert!(cache.get(&key3).is_some());
        assert_eq!(cache.size_bytes(), 20);
    }

    #[test]
    fn test_cache_skips_entry_larger_than_budget() {
        let cache = RenderCache::new(budget(10, 16));
        let small = key("s");
        cache.insert(small.clone(), "ok".to_string());

        cache.insert(key("big"), "b".repeat(64));

        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get(&small), Some("ok".to_string()));
        assert_eq!(cache.size_bytes(), 3);
    }

    #[test]
    fn test_cache_replacement_updates_byte_count() {
        let cache = RenderCache::new(budget(10, 1024));
        let key = key("k");

        cache.insert(key.clone(), "a".repeat(100));
        cache.insert(key.clone(), "b".repeat(10));

        assert_eq!(cache.len(), 1);
        assert_eq!(cache.size_bytes(), 11);
        cache.clear();
        assert_eq!(cache.size_bytes(), 0);
    }

    #[test]
    fn test_cache_budget_follows_resource_limits() {
        let limits = ResourceLimits::default();
        let budget = CacheBudget::from(&limits);
        assert_eq!(budget.max_entries, limits.max_cached_diagrams);
        assert_eq!(budget.max_bytes, limits.max_cache_bytes);
    }
}
