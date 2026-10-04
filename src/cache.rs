use crate::domain::{GraphicsProtocol, ThemeMode};
use std::collections::VecDeque;
use std::sync::{LazyLock, Mutex};

/// Clé composite identifiant de manière unique un diagramme et son contexte de rendu.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RenderCacheKey {
    pub content: String,
    pub theme: ThemeMode,
    pub width: u16,
    pub protocol: GraphicsProtocol,
}

impl RenderCacheKey {
    #[must_use]
    pub fn new(content: &str, theme: ThemeMode, width: u16, protocol: GraphicsProtocol) -> Self {
        Self {
            content: content.to_string(),
            theme,
            width,
            protocol,
        }
    }
}

/// Cache LRU in-process thread-safe pour les rendus terminaux de diagrammes.
#[derive(Debug)]
pub struct RenderCache {
    entries: Mutex<VecDeque<(RenderCacheKey, (String, bool))>>,
    capacity: usize,
}

static GLOBAL_CACHE: LazyLock<RenderCache> = LazyLock::new(|| RenderCache::new(32));

impl RenderCache {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    /// Référence vers le cache global partagé de l'application.
    #[must_use]
    pub fn global() -> &'static Self {
        &GLOBAL_CACHE
    }

    /// Recherche un rendu dans le cache et le replace en tête (LRU).
    #[must_use]
    pub fn get(&self, key: &RenderCacheKey) -> Option<(String, bool)> {
        if self.capacity == 0 {
            return None;
        }

        let mut lock = self.entries.lock().ok()?;
        let idx = lock.iter().position(|(k, _)| k == key)?;
        let entry = lock.remove(idx)?;
        let val = entry.1.clone();
        lock.push_front(entry);
        Some(val)
    }

    /// Insère un rendu dans le cache en appliquant l'éviction LRU si la capacité est atteinte.
    pub fn insert(&self, key: RenderCacheKey, value: (String, bool)) {
        if self.capacity == 0 {
            return;
        }

        let Ok(mut lock) = self.entries.lock() else {
            return;
        };

        if let Some(idx) = lock.iter().position(|(k, _)| k == &key) {
            lock.remove(idx);
        } else if lock.len() >= self.capacity {
            lock.pop_back();
        }

        lock.push_front((key, value));
    }

    /// Vide l'intégralité du cache.
    pub fn clear(&self) {
        if let Ok(mut lock) = self.entries.lock() {
            lock.clear();
        }
    }

    /// Nombre actuel d'entrées en cache.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.lock().map_or(0, |l| l.len())
    }

    /// Indique si le cache est vide.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_insert_and_get() {
        let cache = RenderCache::new(2);
        let key1 = RenderCacheKey::new(
            "graph TD\nA-->B",
            ThemeMode::Dark,
            80,
            GraphicsProtocol::HalfBlocks,
        );
        let key2 = RenderCacheKey::new(
            "graph LR\nA-->B",
            ThemeMode::Light,
            80,
            GraphicsProtocol::Kitty,
        );

        cache.insert(key1.clone(), ("render1".to_string(), true));
        cache.insert(key2.clone(), ("render2".to_string(), true));

        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(&key1), Some(("render1".to_string(), true)));
        assert_eq!(cache.get(&key2), Some(("render2".to_string(), true)));
    }

    #[test]
    fn test_cache_lru_eviction() {
        let cache = RenderCache::new(2);
        let key1 = RenderCacheKey::new("k1", ThemeMode::Dark, 80, GraphicsProtocol::HalfBlocks);
        let key2 = RenderCacheKey::new("k2", ThemeMode::Dark, 80, GraphicsProtocol::HalfBlocks);
        let key3 = RenderCacheKey::new("k3", ThemeMode::Dark, 80, GraphicsProtocol::HalfBlocks);

        cache.insert(key1.clone(), ("r1".to_string(), true));
        cache.insert(key2.clone(), ("r2".to_string(), true));

        // Accès à key1 pour rafraîchir sa position LRU
        let _ = cache.get(&key1);

        // Insertion de key3: doit évincer key2
        cache.insert(key3.clone(), ("r3".to_string(), true));

        assert_eq!(cache.get(&key1), Some(("r1".to_string(), true)));
        assert_eq!(cache.get(&key2), None);
        assert_eq!(cache.get(&key3), Some(("r3".to_string(), true)));
    }

    #[test]
    fn test_cache_zero_capacity() {
        let cache = RenderCache::new(0);
        let key = RenderCacheKey::new("k", ThemeMode::Dark, 80, GraphicsProtocol::HalfBlocks);
        cache.insert(key.clone(), ("r".to_string(), true));
        assert!(cache.is_empty());
        assert_eq!(cache.get(&key), None);
    }
}
