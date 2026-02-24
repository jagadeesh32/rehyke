use crate::config::DelayStrategy;
use rand::seq::SliceRandom;
use rand::Rng;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tracing::debug;

/// Manages anti-detection measures: user agent rotation, browser header
/// profiles, and per-request delay calculation.
pub struct AntiDetect {
    ua_pool: Vec<String>,
    delay_strategy: DelayStrategy,
    /// Current adaptive delay in milliseconds, adjusted based on server feedback.
    adaptive_delay_ms: AtomicU64,
}

impl AntiDetect {
    /// Create a new `AntiDetect` instance with a built-in pool of realistic
    /// user agent strings and the supplied delay strategy.
    pub fn new(delay_strategy: DelayStrategy) -> Self {
        let initial_ms = match &delay_strategy {
            DelayStrategy::Adaptive { initial } => initial.as_millis() as u64,
            _ => 0,
        };
        Self {
            ua_pool: build_ua_pool(),
            delay_strategy,
            adaptive_delay_ms: AtomicU64::new(initial_ms),
        }
    }

    /// Return a randomly selected user agent string from the pool.
    pub fn random_ua(&self) -> &str {
        let mut rng = rand::thread_rng();
        self.ua_pool
            .choose(&mut rng)
            .map(|s| s.as_str())
            .unwrap_or_else(|| {
                debug!("UA pool unexpectedly empty, falling back to default");
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36"
            })
    }

    /// Compute the delay duration before the next request based on the
    /// configured strategy.
    pub fn get_delay(&self) -> Duration {
        match &self.delay_strategy {
            DelayStrategy::Fixed { delay } => {
                debug!(delay_ms = delay.as_millis() as u64, "fixed delay");
                *delay
            }
            DelayStrategy::Random { min, max } => {
                let mut rng = rand::thread_rng();
                let min_ms = min.as_millis() as u64;
                let max_ms = max.as_millis() as u64;
                let ms = if max_ms > min_ms {
                    rng.gen_range(min_ms..=max_ms)
                } else {
                    min_ms
                };
                let d = Duration::from_millis(ms);
                debug!(delay_ms = ms, "random delay");
                d
            }
            DelayStrategy::Adaptive { .. } => {
                let ms = self.adaptive_delay_ms.load(Ordering::Relaxed);
                debug!(delay_ms = ms, "adaptive delay");
                Duration::from_millis(ms)
            }
            DelayStrategy::None => {
                debug!("no delay");
                Duration::ZERO
            }
        }
    }

    /// Record a server response to adjust adaptive delay.
    ///
    /// - On rate-limit responses (429, 503): doubles the delay (up to 30s).
    /// - On server errors (500, 502, 504): increases delay by 50%.
    /// - On success (2xx) with fast response: gradually reduces delay toward
    ///   the initial value (decays by 10%).
    /// - For non-adaptive strategies this is a no-op.
    pub fn record_response(&self, status: u16, _elapsed: Duration) {
        if !matches!(self.delay_strategy, DelayStrategy::Adaptive { .. }) {
            return;
        }

        let initial_ms = match &self.delay_strategy {
            DelayStrategy::Adaptive { initial } => initial.as_millis() as u64,
            _ => return,
        };

        let current = self.adaptive_delay_ms.load(Ordering::Relaxed);
        let max_delay_ms = 30_000u64;

        let new_delay = match status {
            429 | 503 => {
                // Rate limited or service unavailable — double the delay.
                let doubled = current.saturating_mul(2).max(initial_ms);
                debug!(
                    status,
                    old_delay_ms = current,
                    new_delay_ms = doubled.min(max_delay_ms),
                    "adaptive: rate limited, increasing delay"
                );
                doubled.min(max_delay_ms)
            }
            500 | 502 | 504 => {
                // Server error — increase by 50%.
                let increased = current.saturating_add(current / 2).max(initial_ms);
                debug!(
                    status,
                    old_delay_ms = current,
                    new_delay_ms = increased.min(max_delay_ms),
                    "adaptive: server error, increasing delay"
                );
                increased.min(max_delay_ms)
            }
            200..=299 => {
                // Success — decay delay by 10% toward the initial.
                let decayed = current.saturating_sub(current / 10);
                let result = decayed.max(initial_ms);
                if result < current {
                    debug!(
                        old_delay_ms = current,
                        new_delay_ms = result,
                        "adaptive: success, decreasing delay"
                    );
                }
                result
            }
            _ => current,
        };

        self.adaptive_delay_ms.store(new_delay, Ordering::Relaxed);
    }

    /// Return a set of realistic browser headers suitable for inclusion in
    /// every outgoing request.  The returned list includes common headers
    /// that real browsers send, helping the crawler blend in with normal
    /// traffic.
    pub fn browser_headers(&self) -> Vec<(String, String)> {
        let ua = self.random_ua().to_string();
        let mut headers = vec![
            ("User-Agent".to_string(), ua),
            (
                "Accept".to_string(),
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8".to_string(),
            ),
            (
                "Accept-Language".to_string(),
                "en-US,en;q=0.9".to_string(),
            ),
            (
                "Accept-Encoding".to_string(),
                "gzip, deflate, br, zstd".to_string(),
            ),
            ("Connection".to_string(), "keep-alive".to_string()),
            (
                "Upgrade-Insecure-Requests".to_string(),
                "1".to_string(),
            ),
            (
                "Sec-Fetch-Dest".to_string(),
                "document".to_string(),
            ),
            (
                "Sec-Fetch-Mode".to_string(),
                "navigate".to_string(),
            ),
            (
                "Sec-Fetch-Site".to_string(),
                "none".to_string(),
            ),
            (
                "Sec-Fetch-User".to_string(),
                "?1".to_string(),
            ),
            ("DNT".to_string(), "1".to_string()),
            (
                "Sec-Ch-Ua-Platform".to_string(),
                "\"Windows\"".to_string(),
            ),
        ];

        // Randomly decide whether to add an extra realistic header to add
        // per-request variance.
        let mut rng = rand::thread_rng();
        if rng.gen_bool(0.3) {
            headers.push((
                "Cache-Control".to_string(),
                "max-age=0".to_string(),
            ));
        }

        debug!(header_count = headers.len(), "generated browser headers");
        headers
    }
}

/// Build a pool of at least 50 realistic, modern user agent strings spanning
/// Chrome 120+, Firefox 120+, Safari 17+, and Edge 120+ across Windows 10/11,
/// macOS, and Linux.
fn build_ua_pool() -> Vec<String> {
    vec![
        // -- Chrome on Windows 10 --
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36".into(),
        // -- Chrome on macOS --
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".into(),
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36".into(),
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36".into(),
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36".into(),
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36".into(),
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36".into(),
        // -- Chrome on Linux --
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".into(),
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36".into(),
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36".into(),
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36".into(),
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36".into(),
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36".into(),
        // -- Firefox on Windows --
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:120.0) Gecko/20100101 Firefox/120.0".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:122.0) Gecko/20100101 Firefox/122.0".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:123.0) Gecko/20100101 Firefox/123.0".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:124.0) Gecko/20100101 Firefox/124.0".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:125.0) Gecko/20100101 Firefox/125.0".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:126.0) Gecko/20100101 Firefox/126.0".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:127.0) Gecko/20100101 Firefox/127.0".into(),
        // -- Firefox on macOS --
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:120.0) Gecko/20100101 Firefox/120.0".into(),
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:122.0) Gecko/20100101 Firefox/122.0".into(),
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:124.0) Gecko/20100101 Firefox/124.0".into(),
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:126.0) Gecko/20100101 Firefox/126.0".into(),
        // -- Firefox on Linux --
        "Mozilla/5.0 (X11; Linux x86_64; rv:120.0) Gecko/20100101 Firefox/120.0".into(),
        "Mozilla/5.0 (X11; Linux x86_64; rv:122.0) Gecko/20100101 Firefox/122.0".into(),
        "Mozilla/5.0 (X11; Linux x86_64; rv:124.0) Gecko/20100101 Firefox/124.0".into(),
        "Mozilla/5.0 (X11; Linux x86_64; rv:126.0) Gecko/20100101 Firefox/126.0".into(),
        "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:125.0) Gecko/20100101 Firefox/125.0".into(),
        // -- Safari on macOS --
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15".into(),
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_1) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.1 Safari/605.1.15".into(),
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_2) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.2 Safari/605.1.15".into(),
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.3 Safari/605.1.15".into(),
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_4) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15".into(),
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15".into(),
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 13_6) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15".into(),
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 13_5) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.1 Safari/605.1.15".into(),
        // -- Edge on Windows --
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Edg/120.0.0.0".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36 Edg/121.0.0.0".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36 Edg/122.0.0.0".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36 Edg/123.0.0.0".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36 Edg/124.0.0.0".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36 Edg/125.0.0.0".into(),
        // -- Edge on macOS --
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Edg/120.0.0.0".into(),
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36 Edg/122.0.0.0".into(),
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36 Edg/124.0.0.0".into(),
        // -- Chrome on Windows 11 (same NT 10.0 token, higher version) --
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36".into(),
        // -- Edge on Linux --
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Edg/120.0.0.0".into(),
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36 Edg/122.0.0.0".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_ua_pool_has_at_least_50_entries() {
        let pool = build_ua_pool();
        assert!(
            pool.len() >= 50,
            "UA pool should have at least 50 entries, got {}",
            pool.len()
        );
    }

    #[test]
    fn test_ua_pool_entries_are_unique() {
        let pool = build_ua_pool();
        let unique: HashSet<&str> = pool.iter().map(|s| s.as_str()).collect();
        assert_eq!(pool.len(), unique.len(), "UA pool contains duplicates");
    }

    #[test]
    fn test_ua_pool_includes_chrome() {
        let pool = build_ua_pool();
        let count = pool.iter().filter(|ua| ua.contains("Chrome/")).count();
        assert!(count >= 10, "Expected at least 10 Chrome UAs, got {}", count);
    }

    #[test]
    fn test_ua_pool_includes_firefox() {
        let pool = build_ua_pool();
        let count = pool.iter().filter(|ua| ua.contains("Firefox/")).count();
        assert!(count >= 5, "Expected at least 5 Firefox UAs, got {}", count);
    }

    #[test]
    fn test_ua_pool_includes_safari() {
        let pool = build_ua_pool();
        let count = pool
            .iter()
            .filter(|ua| ua.contains("Version/17") && ua.contains("Safari/"))
            .count();
        assert!(count >= 4, "Expected at least 4 Safari 17+ UAs, got {}", count);
    }

    #[test]
    fn test_ua_pool_includes_edge() {
        let pool = build_ua_pool();
        let count = pool.iter().filter(|ua| ua.contains("Edg/")).count();
        assert!(count >= 5, "Expected at least 5 Edge UAs, got {}", count);
    }

    #[test]
    fn test_ua_pool_includes_windows() {
        let pool = build_ua_pool();
        assert!(pool.iter().any(|ua| ua.contains("Windows NT")));
    }

    #[test]
    fn test_ua_pool_includes_macos() {
        let pool = build_ua_pool();
        assert!(pool.iter().any(|ua| ua.contains("Macintosh")));
    }

    #[test]
    fn test_ua_pool_includes_linux() {
        let pool = build_ua_pool();
        assert!(pool.iter().any(|ua| ua.contains("Linux")));
    }

    #[test]
    fn test_random_ua_returns_nonempty() {
        let ad = AntiDetect::new(DelayStrategy::default());
        let ua = ad.random_ua();
        assert!(!ua.is_empty());
        assert!(ua.contains("Mozilla"));
    }

    #[test]
    fn test_fixed_delay() {
        let ad = AntiDetect::new(DelayStrategy::Fixed {
            delay: Duration::from_millis(200),
        });
        let d = ad.get_delay();
        assert_eq!(d, Duration::from_millis(200));
    }

    #[test]
    fn test_random_delay_within_bounds() {
        let min = Duration::from_millis(100);
        let max = Duration::from_millis(500);
        let ad = AntiDetect::new(DelayStrategy::Random { min, max });
        for _ in 0..100 {
            let d = ad.get_delay();
            assert!(d >= min, "delay {:?} < min {:?}", d, min);
            assert!(d <= max, "delay {:?} > max {:?}", d, max);
        }
    }

    #[test]
    fn test_adaptive_delay_returns_initial() {
        let initial = Duration::from_millis(150);
        let ad = AntiDetect::new(DelayStrategy::Adaptive { initial });
        let d = ad.get_delay();
        assert_eq!(d, initial);
    }

    #[test]
    fn test_adaptive_delay_increases_on_429() {
        let initial = Duration::from_millis(100);
        let ad = AntiDetect::new(DelayStrategy::Adaptive { initial });
        assert_eq!(ad.get_delay(), initial);

        ad.record_response(429, Duration::from_millis(500));
        let after = ad.get_delay();
        assert!(after > initial, "delay should increase after 429, got {:?}", after);
        assert_eq!(after, Duration::from_millis(200)); // doubled
    }

    #[test]
    fn test_adaptive_delay_increases_on_503() {
        let initial = Duration::from_millis(100);
        let ad = AntiDetect::new(DelayStrategy::Adaptive { initial });

        ad.record_response(503, Duration::from_millis(500));
        let after = ad.get_delay();
        assert_eq!(after, Duration::from_millis(200)); // doubled
    }

    #[test]
    fn test_adaptive_delay_increases_on_500() {
        let initial = Duration::from_millis(100);
        let ad = AntiDetect::new(DelayStrategy::Adaptive { initial });

        ad.record_response(500, Duration::from_millis(500));
        let after = ad.get_delay();
        assert_eq!(after, Duration::from_millis(150)); // +50%
    }

    #[test]
    fn test_adaptive_delay_decays_on_success() {
        let initial = Duration::from_millis(100);
        let ad = AntiDetect::new(DelayStrategy::Adaptive { initial });

        // First ramp up.
        ad.record_response(429, Duration::from_millis(500));
        assert_eq!(ad.get_delay(), Duration::from_millis(200));

        // Then decay on success.
        ad.record_response(200, Duration::from_millis(100));
        let after = ad.get_delay();
        assert!(after < Duration::from_millis(200), "delay should decay on success");
        assert!(after >= initial, "delay should not go below initial");
    }

    #[test]
    fn test_adaptive_delay_capped_at_30s() {
        let initial = Duration::from_millis(10_000);
        let ad = AntiDetect::new(DelayStrategy::Adaptive { initial });

        // Double to 20s.
        ad.record_response(429, Duration::from_millis(500));
        assert_eq!(ad.get_delay(), Duration::from_millis(20_000));

        // Double to 40s, but capped at 30s.
        ad.record_response(429, Duration::from_millis(500));
        assert_eq!(ad.get_delay(), Duration::from_secs(30));
    }

    #[test]
    fn test_record_response_noop_for_non_adaptive() {
        let ad = AntiDetect::new(DelayStrategy::Fixed {
            delay: Duration::from_millis(100),
        });
        // Should not panic or change anything.
        ad.record_response(429, Duration::from_millis(500));
        assert_eq!(ad.get_delay(), Duration::from_millis(100));
    }

    #[test]
    fn test_none_delay_returns_zero() {
        let ad = AntiDetect::new(DelayStrategy::None);
        let d = ad.get_delay();
        assert_eq!(d, Duration::ZERO);
    }

    #[test]
    fn test_browser_headers_include_required_keys() {
        let ad = AntiDetect::new(DelayStrategy::default());
        let headers = ad.browser_headers();
        let keys: Vec<&str> = headers.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"User-Agent"));
        assert!(keys.contains(&"Accept"));
        assert!(keys.contains(&"Accept-Language"));
        assert!(keys.contains(&"Accept-Encoding"));
        assert!(keys.contains(&"Connection"));
    }

    #[test]
    fn test_browser_headers_user_agent_is_from_pool() {
        let ad = AntiDetect::new(DelayStrategy::default());
        let headers = ad.browser_headers();
        let ua_value = headers
            .iter()
            .find(|(k, _)| k == "User-Agent")
            .map(|(_, v)| v.as_str())
            .unwrap();
        assert!(ad.ua_pool.iter().any(|u| u == ua_value));
    }

    #[test]
    fn test_random_delay_min_equals_max() {
        let d = Duration::from_millis(250);
        let ad = AntiDetect::new(DelayStrategy::Random { min: d, max: d });
        assert_eq!(ad.get_delay(), d);
    }
}
