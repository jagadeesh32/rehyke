use crate::config::{ProxyAuth, ProxyConfig, ProxyStrategy, ProxyType};
use rand::Rng;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::debug;

/// A pool of proxies with configurable rotation strategies.
pub struct ProxyPool {
    proxies: Vec<ProxyConfig>,
    strategy: ProxyStrategy,
    current_index: AtomicUsize,
}

impl ProxyPool {
    /// Create a new proxy pool with the given list of proxies and selection
    /// strategy.
    pub fn new(proxies: Vec<ProxyConfig>, strategy: ProxyStrategy) -> Self {
        debug!(
            proxy_count = proxies.len(),
            strategy = ?strategy,
            "created proxy pool"
        );
        Self {
            proxies,
            strategy,
            current_index: AtomicUsize::new(0),
        }
    }

    /// Select the next proxy according to the configured strategy.
    ///
    /// Returns `None` when the pool is empty.
    pub fn next_proxy(&self) -> Option<&ProxyConfig> {
        if self.proxies.is_empty() {
            return None;
        }

        match self.strategy {
            ProxyStrategy::RoundRobin => {
                let idx = self.current_index.fetch_add(1, Ordering::Relaxed);
                let proxy = &self.proxies[idx % self.proxies.len()];
                debug!(
                    index = idx % self.proxies.len(),
                    url = %proxy.url,
                    "round-robin proxy selected"
                );
                Some(proxy)
            }
            ProxyStrategy::Random => {
                let mut rng = rand::thread_rng();
                let idx = rng.gen_range(0..self.proxies.len());
                let proxy = &self.proxies[idx];
                debug!(
                    index = idx,
                    url = %proxy.url,
                    "random proxy selected"
                );
                Some(proxy)
            }
            ProxyStrategy::LeastUsed => {
                // Without per-proxy usage counters, LeastUsed falls back to
                // round-robin.  Full implementation would require an
                // AtomicUsize per proxy tracking in-flight requests.
                let idx = self.current_index.fetch_add(1, Ordering::Relaxed);
                let proxy = &self.proxies[idx % self.proxies.len()];
                debug!(
                    index = idx % self.proxies.len(),
                    url = %proxy.url,
                    "least-used proxy selected (round-robin fallback)"
                );
                Some(proxy)
            }
            ProxyStrategy::FailoverOnly => {
                // In failover mode the first proxy is the primary.  The
                // caller is responsible for cycling to the next proxy upon
                // failure.  Here we always return the current index which
                // starts at 0.
                let idx = self.current_index.load(Ordering::Relaxed);
                let proxy = &self.proxies[idx % self.proxies.len()];
                debug!(
                    index = idx % self.proxies.len(),
                    url = %proxy.url,
                    "failover proxy selected"
                );
                Some(proxy)
            }
        }
    }

    /// Advance the failover index to the next proxy.  This is intended to
    /// be called by the engine when the current proxy fails and the
    /// strategy is `FailoverOnly`.
    pub fn advance_failover(&self) {
        self.current_index.fetch_add(1, Ordering::Relaxed);
    }

    /// Return `true` if the pool contains no proxies.
    pub fn is_empty(&self) -> bool {
        self.proxies.is_empty()
    }

    /// Return the number of proxies in the pool.
    pub fn len(&self) -> usize {
        self.proxies.len()
    }
}

impl ProxyConfig {
    /// Convert this proxy configuration into a `reqwest::Proxy`.
    ///
    /// Applies authentication if credentials are present.
    pub fn to_reqwest_proxy(&self) -> Result<reqwest::Proxy, reqwest::Error> {
        let proxy = match self.proxy_type {
            ProxyType::Http => reqwest::Proxy::http(&self.url)?,
            ProxyType::Https => reqwest::Proxy::https(&self.url)?,
            ProxyType::Socks5 => reqwest::Proxy::all(&self.url)?,
        };

        let proxy = if let Some(ref credentials) = self.auth {
            proxy.basic_auth(&credentials.username, &credentials.password)
        } else {
            proxy
        };

        Ok(proxy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_proxy(url: &str) -> ProxyConfig {
        ProxyConfig {
            url: url.to_string(),
            proxy_type: ProxyType::Http,
            auth: None,
            region: None,
        }
    }

    // -----------------------------------------------------------------------
    // Round-robin strategy
    // -----------------------------------------------------------------------

    #[test]
    fn test_round_robin_cycles() {
        let proxies = vec![
            make_proxy("http://proxy1:8080"),
            make_proxy("http://proxy2:8080"),
            make_proxy("http://proxy3:8080"),
        ];
        let pool = ProxyPool::new(proxies, ProxyStrategy::RoundRobin);

        assert_eq!(pool.next_proxy().unwrap().url, "http://proxy1:8080");
        assert_eq!(pool.next_proxy().unwrap().url, "http://proxy2:8080");
        assert_eq!(pool.next_proxy().unwrap().url, "http://proxy3:8080");
        // Wraps around.
        assert_eq!(pool.next_proxy().unwrap().url, "http://proxy1:8080");
        assert_eq!(pool.next_proxy().unwrap().url, "http://proxy2:8080");
    }

    #[test]
    fn test_round_robin_single_proxy() {
        let proxies = vec![make_proxy("http://only:8080")];
        let pool = ProxyPool::new(proxies, ProxyStrategy::RoundRobin);
        for _ in 0..5 {
            assert_eq!(pool.next_proxy().unwrap().url, "http://only:8080");
        }
    }

    // -----------------------------------------------------------------------
    // Random strategy
    // -----------------------------------------------------------------------

    #[test]
    fn test_random_returns_valid_proxy() {
        let urls = vec!["http://proxy1:8080", "http://proxy2:8080", "http://proxy3:8080"];
        let proxies = urls.iter().map(|u| make_proxy(u)).collect();
        let pool = ProxyPool::new(proxies, ProxyStrategy::Random);

        for _ in 0..50 {
            let selected = pool.next_proxy().unwrap();
            assert!(
                urls.contains(&selected.url.as_str()),
                "unexpected proxy URL: {}",
                selected.url
            );
        }
    }

    #[test]
    fn test_random_single_proxy() {
        let proxies = vec![make_proxy("http://only:8080")];
        let pool = ProxyPool::new(proxies, ProxyStrategy::Random);
        assert_eq!(pool.next_proxy().unwrap().url, "http://only:8080");
    }

    // -----------------------------------------------------------------------
    // Failover strategy
    // -----------------------------------------------------------------------

    #[test]
    fn test_failover_starts_at_first() {
        let proxies = vec![
            make_proxy("http://primary:8080"),
            make_proxy("http://backup:8080"),
        ];
        let pool = ProxyPool::new(proxies, ProxyStrategy::FailoverOnly);

        assert_eq!(pool.next_proxy().unwrap().url, "http://primary:8080");
        assert_eq!(pool.next_proxy().unwrap().url, "http://primary:8080");
    }

    #[test]
    fn test_failover_advance() {
        let proxies = vec![
            make_proxy("http://primary:8080"),
            make_proxy("http://backup1:8080"),
            make_proxy("http://backup2:8080"),
        ];
        let pool = ProxyPool::new(proxies, ProxyStrategy::FailoverOnly);

        assert_eq!(pool.next_proxy().unwrap().url, "http://primary:8080");
        pool.advance_failover();
        assert_eq!(pool.next_proxy().unwrap().url, "http://backup1:8080");
        pool.advance_failover();
        assert_eq!(pool.next_proxy().unwrap().url, "http://backup2:8080");
        pool.advance_failover();
        // Wraps around.
        assert_eq!(pool.next_proxy().unwrap().url, "http://primary:8080");
    }

    // -----------------------------------------------------------------------
    // Empty pool
    // -----------------------------------------------------------------------

    #[test]
    fn test_empty_pool() {
        let pool = ProxyPool::new(Vec::new(), ProxyStrategy::RoundRobin);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert!(pool.next_proxy().is_none());
    }

    #[test]
    fn test_empty_pool_random() {
        let pool = ProxyPool::new(Vec::new(), ProxyStrategy::Random);
        assert!(pool.next_proxy().is_none());
    }

    #[test]
    fn test_empty_pool_failover() {
        let pool = ProxyPool::new(Vec::new(), ProxyStrategy::FailoverOnly);
        assert!(pool.next_proxy().is_none());
    }

    // -----------------------------------------------------------------------
    // Pool metadata
    // -----------------------------------------------------------------------

    #[test]
    fn test_len_and_is_empty() {
        let pool = ProxyPool::new(
            vec![make_proxy("http://a:80"), make_proxy("http://b:80")],
            ProxyStrategy::RoundRobin,
        );
        assert!(!pool.is_empty());
        assert_eq!(pool.len(), 2);
    }

    // -----------------------------------------------------------------------
    // ProxyConfig to reqwest::Proxy
    // -----------------------------------------------------------------------

    #[test]
    fn test_to_reqwest_proxy_http() {
        let config = ProxyConfig {
            url: "http://proxy.example.com:8080".to_string(),
            proxy_type: ProxyType::Http,
            auth: None,
            region: None,
        };
        let result = config.to_reqwest_proxy();
        assert!(result.is_ok());
    }

    #[test]
    fn test_to_reqwest_proxy_https() {
        let config = ProxyConfig {
            url: "https://proxy.example.com:8443".to_string(),
            proxy_type: ProxyType::Https,
            auth: None,
            region: None,
        };
        let result = config.to_reqwest_proxy();
        assert!(result.is_ok());
    }

    #[test]
    fn test_to_reqwest_proxy_socks5() {
        let config = ProxyConfig {
            url: "socks5://proxy.example.com:1080".to_string(),
            proxy_type: ProxyType::Socks5,
            auth: None,
            region: None,
        };
        let result = config.to_reqwest_proxy();
        assert!(result.is_ok());
    }

    #[test]
    fn test_to_reqwest_proxy_with_auth() {
        let config = ProxyConfig {
            url: "http://proxy.example.com:8080".to_string(),
            proxy_type: ProxyType::Http,
            auth: Some(ProxyAuth {
                username: "user".to_string(),
                password: "pass".to_string(),
            }),
            region: None,
        };
        let result = config.to_reqwest_proxy();
        assert!(result.is_ok());
    }

    #[test]
    fn test_proxy_config_region() {
        let config = ProxyConfig {
            url: "http://proxy.example.com:8080".to_string(),
            proxy_type: ProxyType::Http,
            auth: None,
            region: Some("us-east-1".to_string()),
        };
        assert_eq!(config.region.as_deref(), Some("us-east-1"));
    }
}
