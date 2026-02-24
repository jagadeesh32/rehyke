use crate::config::{CrawlConfig, ScanMode};
use crate::utils;
use dashmap::{DashMap, DashSet};
use std::collections::BinaryHeap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};
use url::Url;

// ---------------------------------------------------------------------------
// Priority & TaskSource
// ---------------------------------------------------------------------------

/// Priority levels for crawl tasks.
///
/// Higher numeric value = higher priority. The `BinaryHeap` is a max-heap,
/// so [`Priority::Critical`] tasks are dequeued before [`Priority::Low`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Source of how a URL was discovered.
#[derive(Debug, Clone)]
pub enum TaskSource {
    /// Initial URL provided by the user.
    Seed,
    /// Found on an internal page.
    InternalLink,
    /// Found as an external link.
    ExternalLink,
    /// Found in sitemap.xml.
    Sitemap,
    /// Found in an RSS/Atom feed.
    Feed,
}

// ---------------------------------------------------------------------------
// CrawlTask
// ---------------------------------------------------------------------------

/// A single crawl task in the queue.
#[derive(Debug, Clone)]
pub struct CrawlTask {
    pub url: Url,
    pub depth: u32,
    pub priority: Priority,
    pub source: TaskSource,
    pub requires_js: bool,
}

impl PartialEq for CrawlTask {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl Eq for CrawlTask {}

impl PartialOrd for CrawlTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CrawlTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Higher priority first (BinaryHeap is a max-heap).
        self.priority.cmp(&other.priority)
    }
}

// ---------------------------------------------------------------------------
// CrawlStats
// ---------------------------------------------------------------------------

/// Statistics about the crawl progress.
#[derive(Debug, Default)]
pub struct CrawlStats {
    pub total_discovered: AtomicUsize,
    pub total_crawled: AtomicUsize,
    pub total_errors: AtomicUsize,
    pub total_skipped: AtomicUsize,
}

impl CrawlStats {
    /// Take a consistent snapshot of the current counters.
    pub fn snapshot(&self) -> CrawlStatsSnapshot {
        CrawlStatsSnapshot {
            total_discovered: self.total_discovered.load(Ordering::Relaxed),
            total_crawled: self.total_crawled.load(Ordering::Relaxed),
            total_errors: self.total_errors.load(Ordering::Relaxed),
            total_skipped: self.total_skipped.load(Ordering::Relaxed),
        }
    }
}

/// An immutable, cloneable snapshot of [`CrawlStats`].
#[derive(Debug, Clone)]
pub struct CrawlStatsSnapshot {
    pub total_discovered: usize,
    pub total_crawled: usize,
    pub total_errors: usize,
    pub total_skipped: usize,
}

// ---------------------------------------------------------------------------
// Scheduler
// ---------------------------------------------------------------------------

/// The main scheduler that manages the crawl frontier.
///
/// It owns a priority queue of pending [`CrawlTask`]s, tracks visited URLs for
/// deduplication, enforces per-domain rate limits, and respects the configured
/// [`ScanMode`] to decide which discovered URLs should be enqueued.
pub struct Scheduler {
    /// Priority queue of URLs to crawl.
    frontier: Mutex<BinaryHeap<CrawlTask>>,
    /// Set of already-visited URLs (normalized) for dedup.
    visited: DashSet<String>,
    /// Set of URLs currently being fetched.
    in_progress: DashSet<String>,
    /// Per-domain last-request timestamps for rate limiting.
    domain_delays: DashMap<String, Instant>,
    /// Crawl statistics.
    pub stats: Arc<CrawlStats>,
    /// Maximum number of pages to crawl.
    max_pages: usize,
    /// Maximum crawl depth.
    max_depth: u32,
    /// Scan mode controlling which URLs are accepted.
    mode: ScanMode,
    /// Minimum delay between requests to the same domain.
    domain_delay: Duration,
    /// Whether to strip `www.` during URL normalization.
    remove_www: bool,
    /// Whether the scheduler has been explicitly marked as done.
    done: AtomicBool,
    /// The seed URL, stored after `add_seed` for same-domain checks.
    seed_url: Mutex<Option<Url>>,
}

impl Scheduler {
    /// Create a new `Scheduler` from the given [`CrawlConfig`] and [`ScanMode`].
    pub fn new(config: &CrawlConfig, mode: ScanMode) -> Self {
        let max_pages = config.max_pages;

        let max_depth = config.max_depth as u32;

        // Extract a per-domain delay from the delay strategy.
        let domain_delay = match &config.delay_strategy {
            crate::config::DelayStrategy::Fixed { delay } => *delay,
            crate::config::DelayStrategy::Random { min, .. } => *min,
            crate::config::DelayStrategy::Adaptive { initial } => *initial,
            crate::config::DelayStrategy::None => Duration::ZERO,
        };

        info!(
            mode = ?mode,
            max_pages = max_pages,
            max_depth = max_depth,
            domain_delay_ms = domain_delay.as_millis(),
            "Scheduler initialised"
        );

        Self {
            frontier: Mutex::new(BinaryHeap::new()),
            visited: DashSet::new(),
            in_progress: DashSet::new(),
            domain_delays: DashMap::new(),
            stats: Arc::new(CrawlStats::default()),
            max_pages,
            max_depth,
            mode,
            domain_delay,
            remove_www: true,
            done: AtomicBool::new(false),
            seed_url: Mutex::new(None),
        }
    }

    // ------------------------------------------------------------------
    // Seed handling
    // ------------------------------------------------------------------

    /// Add the initial seed URL with [`Priority::Critical`].
    pub fn add_seed(&self, url: Url) {
        let normalized = utils::normalize_url(&url, self.remove_www);

        // Store the seed so we can do same-domain comparisons later.
        {
            let mut seed = self.seed_url.lock().expect("seed_url lock poisoned");
            *seed = Some(url.clone());
        }

        // Mark as discovered.
        self.visited.insert(normalized.clone());
        self.stats.total_discovered.fetch_add(1, Ordering::Relaxed);

        let task = CrawlTask {
            url,
            depth: 0,
            priority: Priority::Critical,
            source: TaskSource::Seed,
            requires_js: false,
        };

        let mut queue = self.frontier.lock().expect("frontier lock poisoned");
        queue.push(task);

        info!(normalized = %normalized, "Seed URL added to frontier");
    }

    // ------------------------------------------------------------------
    // Bulk URL insertion
    // ------------------------------------------------------------------

    /// Add a batch of discovered URLs to the frontier.
    ///
    /// Each URL is normalised and deduplicated. Depending on the current
    /// [`ScanMode`], some or all URLs may be silently dropped:
    ///
    /// - **Lite** -- no URLs are added (single-page mode).
    /// - **Full** -- only same-domain (internal) URLs are added.
    /// - **Deep** -- both internal and external URLs are added.
    pub fn add_urls(&self, urls: Vec<Url>, depth: u32, source: TaskSource) {
        // Lite mode: never follow links.
        if self.mode == ScanMode::Lite {
            debug!("Lite mode: skipping {} discovered URLs", urls.len());
            return;
        }

        // We need the seed URL for same-domain checks.
        let seed = {
            let guard = self.seed_url.lock().expect("seed_url lock poisoned");
            match guard.clone() {
                Some(s) => s,
                None => {
                    warn!("add_urls called before add_seed; dropping URLs");
                    return;
                }
            }
        };

        for url in urls {
            self.try_enqueue(url, depth, &source, &seed);
        }
    }

    /// Attempt to enqueue a single URL. Returns `true` if the URL was added.
    fn try_enqueue(&self, url: Url, depth: u32, source: &TaskSource, seed: &Url) -> bool {
        // --- depth check ---
        if depth > self.max_depth {
            debug!(url = %url, depth, max = self.max_depth, "Skipping: exceeds max depth");
            self.stats.total_skipped.fetch_add(1, Ordering::Relaxed);
            return false;
        }

        // --- max pages check ---
        let discovered = self.stats.total_discovered.load(Ordering::Relaxed);
        if discovered >= self.max_pages {
            debug!(url = %url, "Skipping: max pages limit reached");
            self.stats.total_skipped.fetch_add(1, Ordering::Relaxed);
            return false;
        }

        // --- domain / mode check ---
        let is_internal = utils::is_same_domain(&url, seed);
        match self.mode {
            ScanMode::Lite => {
                // Should never reach here (handled in add_urls), but be safe.
                return false;
            }
            ScanMode::Full => {
                if !is_internal {
                    debug!(url = %url, "Skipping external URL in Full mode");
                    self.stats.total_skipped.fetch_add(1, Ordering::Relaxed);
                    return false;
                }
            }
            ScanMode::Deep => {
                // Allow both internal and external.
            }
        }

        // --- normalize & dedup ---
        let normalized = utils::normalize_url(&url, self.remove_www);

        if self.visited.contains(&normalized) || self.in_progress.contains(&normalized) {
            debug!(url = %normalized, "Skipping: already visited or in-progress");
            return false;
        }

        // Insert into visited immediately so concurrent callers don't double-add.
        if !self.visited.insert(normalized.clone()) {
            // Another thread beat us to it.
            return false;
        }

        self.stats.total_discovered.fetch_add(1, Ordering::Relaxed);

        // --- assign priority ---
        let priority = Self::compute_priority(source, depth, is_internal);

        let task = CrawlTask {
            url,
            depth,
            priority,
            source: source.clone(),
            requires_js: false,
        };

        let mut queue = self.frontier.lock().expect("frontier lock poisoned");
        queue.push(task);

        debug!(
            normalized = %normalized,
            depth,
            priority = ?priority,
            "URL added to frontier"
        );

        true
    }

    /// Determine the priority of a task based on its source, depth, and whether
    /// it belongs to the seed domain.
    fn compute_priority(source: &TaskSource, depth: u32, is_internal: bool) -> Priority {
        match source {
            TaskSource::Seed => Priority::Critical,
            TaskSource::Sitemap | TaskSource::Feed => Priority::Critical,
            TaskSource::InternalLink => {
                if depth <= 1 {
                    Priority::High
                } else {
                    Priority::Normal
                }
            }
            TaskSource::ExternalLink => {
                if is_internal {
                    // Misclassified, but still treat as normal internal link.
                    Priority::Normal
                } else {
                    Priority::Low
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Task retrieval
    // ------------------------------------------------------------------

    /// Get the next URL to crawl from the priority queue.
    ///
    /// Returns `None` when the queue is empty. If the highest-priority task's
    /// domain was accessed too recently (rate limiting), the task is re-queued
    /// and the method attempts the next task. After exhausting a bounded number
    /// of attempts it returns `None` to avoid busy-looping.
    pub fn next_task(&self) -> Option<CrawlTask> {
        let mut queue = self.frontier.lock().expect("frontier lock poisoned");

        // We will try at most `queue.len()` times to find a non-rate-limited
        // task before giving up.
        let max_attempts = queue.len();
        let mut deferred: Vec<CrawlTask> = Vec::new();

        for _ in 0..max_attempts {
            let task = match queue.pop() {
                Some(t) => t,
                None => break,
            };

            let domain = task.url.host_str().unwrap_or("unknown").to_lowercase();

            if self.is_rate_limited(&domain) {
                debug!(domain = %domain, "Domain rate-limited, deferring task");
                deferred.push(task);
                continue;
            }

            // Put all deferred tasks back.
            for d in deferred {
                queue.push(d);
            }

            // Mark as in-progress.
            let normalized = utils::normalize_url(&task.url, self.remove_www);
            self.in_progress.insert(normalized);

            return Some(task);
        }

        // Everything we tried was rate-limited; push them all back.
        for d in deferred {
            queue.push(d);
        }

        None
    }

    /// Check whether a domain has been accessed too recently.
    fn is_rate_limited(&self, domain: &str) -> bool {
        if self.domain_delay.is_zero() {
            return false;
        }
        if let Some(last) = self.domain_delays.get(domain) {
            last.elapsed() < self.domain_delay
        } else {
            false
        }
    }

    // ------------------------------------------------------------------
    // Completion tracking
    // ------------------------------------------------------------------

    /// Mark a URL as successfully crawled.
    pub fn mark_completed(&self, url: &Url) {
        let normalized = utils::normalize_url(url, self.remove_www);
        self.in_progress.remove(&normalized);
        // `visited` was already inserted during enqueue, but ensure it is present.
        self.visited.insert(normalized);

        // Update the per-domain timestamp for rate limiting.
        let domain = url.host_str().unwrap_or("unknown").to_lowercase();
        self.domain_delays.insert(domain, Instant::now());

        self.stats.total_crawled.fetch_add(1, Ordering::Relaxed);

        debug!(url = %url, "Marked as completed");
    }

    /// Mark a URL as failed (will not be retried).
    pub fn mark_failed(&self, url: &Url) {
        let normalized = utils::normalize_url(url, self.remove_www);
        self.in_progress.remove(&normalized);
        // Insert into visited so we don't try again.
        self.visited.insert(normalized);

        self.stats.total_errors.fetch_add(1, Ordering::Relaxed);

        warn!(url = %url, "Marked as failed");
    }

    // ------------------------------------------------------------------
    // Query methods
    // ------------------------------------------------------------------

    /// Returns `true` when the crawl is complete: the frontier is empty **and**
    /// there are no tasks in progress.
    pub fn is_done(&self) -> bool {
        if self.done.load(Ordering::Relaxed) {
            return true;
        }

        let queue_empty = {
            let queue = self.frontier.lock().expect("frontier lock poisoned");
            queue.is_empty()
        };

        queue_empty && self.in_progress.is_empty()
    }

    /// Number of URLs currently waiting in the priority queue.
    pub fn pending_count(&self) -> usize {
        let queue = self.frontier.lock().expect("frontier lock poisoned");
        queue.len()
    }

    /// Check if a URL should be crawled given the current mode, depth limits,
    /// and deduplication state.
    ///
    /// This is a read-only query that does **not** mutate any internal state.
    pub fn should_crawl(&self, url: &Url) -> bool {
        // Already seen?
        let normalized = utils::normalize_url(url, self.remove_www);
        if self.visited.contains(&normalized) || self.in_progress.contains(&normalized) {
            return false;
        }

        // Max pages reached?
        let discovered = self.stats.total_discovered.load(Ordering::Relaxed);
        if discovered >= self.max_pages {
            return false;
        }

        // Mode-based domain filtering.
        let seed = {
            let guard = self.seed_url.lock().expect("seed_url lock poisoned");
            guard.clone()
        };

        if let Some(seed) = seed {
            let is_internal = utils::is_same_domain(url, &seed);
            match self.mode {
                ScanMode::Lite => return false,
                ScanMode::Full => {
                    if !is_internal {
                        return false;
                    }
                }
                ScanMode::Deep => {}
            }
        } else {
            // No seed yet -- we can't make a decision about internal/external.
            return false;
        }

        true
    }

    /// Explicitly mark the scheduler as done (e.g. on cancellation).
    pub fn set_done(&self) {
        self.done.store(true, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CrawlConfig;

    /// Helper: build a scheduler with zero domain delay for fast tests.
    fn make_fast_scheduler(mode: ScanMode) -> Scheduler {
        let config = CrawlConfig {
            delay_strategy: crate::config::DelayStrategy::Fixed { delay: Duration::ZERO },
            ..CrawlConfig::default()
        };
        Scheduler::new(&config, mode)
    }

    fn url(s: &str) -> Url {
        Url::parse(s).expect("valid test URL")
    }

    // ------------------------------------------------------------------
    // Seed URL
    // ------------------------------------------------------------------

    #[test]
    fn test_add_seed() {
        let sched = make_fast_scheduler(ScanMode::Full);
        sched.add_seed(url("https://example.com"));

        assert_eq!(sched.pending_count(), 1);
        assert_eq!(sched.stats.total_discovered.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_seed_is_critical_priority() {
        let sched = make_fast_scheduler(ScanMode::Full);
        sched.add_seed(url("https://example.com"));

        let task = sched.next_task().expect("should have a task");
        assert_eq!(task.priority, Priority::Critical);
        assert_eq!(task.depth, 0);
    }

    // ------------------------------------------------------------------
    // URL deduplication
    // ------------------------------------------------------------------

    #[test]
    fn test_dedup_same_url() {
        let sched = make_fast_scheduler(ScanMode::Full);
        sched.add_seed(url("https://example.com"));

        sched.add_urls(
            vec![
                url("https://example.com/page"),
                url("https://example.com/page"),
                url("https://example.com/page"),
            ],
            1,
            TaskSource::InternalLink,
        );

        // Seed + one unique page = 2 discovered.
        assert_eq!(sched.stats.total_discovered.load(Ordering::Relaxed), 2);
        // Queue: seed + page = 2 (seed not yet popped).
        assert_eq!(sched.pending_count(), 2);
    }

    #[test]
    fn test_dedup_normalised_urls() {
        let sched = make_fast_scheduler(ScanMode::Full);
        sched.add_seed(url("https://example.com"));

        sched.add_urls(
            vec![
                url("https://example.com/page#section1"),
                url("https://example.com/page#section2"),
            ],
            1,
            TaskSource::InternalLink,
        );

        // Fragments are stripped during normalization, so both resolve to the same URL.
        assert_eq!(sched.stats.total_discovered.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_dedup_visited_url_not_re_added() {
        let sched = make_fast_scheduler(ScanMode::Full);
        sched.add_seed(url("https://example.com"));

        // Pop the seed and mark it completed.
        let task = sched.next_task().unwrap();
        sched.mark_completed(&task.url);

        // Try adding the same URL again.
        sched.add_urls(
            vec![url("https://example.com")],
            1,
            TaskSource::InternalLink,
        );

        // Should not be re-added to the queue.
        assert_eq!(sched.pending_count(), 0);
    }

    // ------------------------------------------------------------------
    // Priority ordering
    // ------------------------------------------------------------------

    #[test]
    fn test_priority_ordering_critical_first() {
        // BinaryHeap should give us Critical before High before Normal before Low.
        let mut heap = BinaryHeap::new();
        heap.push(CrawlTask {
            url: url("https://example.com/low"),
            depth: 5,
            priority: Priority::Low,
            source: TaskSource::ExternalLink,
            requires_js: false,
        });
        heap.push(CrawlTask {
            url: url("https://example.com/critical"),
            depth: 0,
            priority: Priority::Critical,
            source: TaskSource::Seed,
            requires_js: false,
        });
        heap.push(CrawlTask {
            url: url("https://example.com/normal"),
            depth: 3,
            priority: Priority::Normal,
            source: TaskSource::InternalLink,
            requires_js: false,
        });
        heap.push(CrawlTask {
            url: url("https://example.com/high"),
            depth: 1,
            priority: Priority::High,
            source: TaskSource::InternalLink,
            requires_js: false,
        });

        assert_eq!(heap.pop().unwrap().priority, Priority::Critical);
        assert_eq!(heap.pop().unwrap().priority, Priority::High);
        assert_eq!(heap.pop().unwrap().priority, Priority::Normal);
        assert_eq!(heap.pop().unwrap().priority, Priority::Low);
    }

    #[test]
    fn test_scheduler_dequeues_higher_priority_first() {
        let sched = make_fast_scheduler(ScanMode::Deep);
        sched.add_seed(url("https://example.com"));

        // Add an external (Low) and an internal (High/Normal) link.
        sched.add_urls(
            vec![url("https://other.com/external")],
            1,
            TaskSource::ExternalLink,
        );
        sched.add_urls(
            vec![url("https://example.com/internal")],
            1,
            TaskSource::InternalLink,
        );

        // First dequeue: seed (Critical).
        let t1 = sched.next_task().unwrap();
        assert_eq!(t1.priority, Priority::Critical);

        // Second dequeue: internal link at depth 1 (High).
        let t2 = sched.next_task().unwrap();
        assert_eq!(t2.priority, Priority::High);

        // Third dequeue: external link (Low).
        let t3 = sched.next_task().unwrap();
        assert_eq!(t3.priority, Priority::Low);
    }

    // ------------------------------------------------------------------
    // Mode filtering
    // ------------------------------------------------------------------

    #[test]
    fn test_lite_mode_blocks_all() {
        let sched = make_fast_scheduler(ScanMode::Lite);
        sched.add_seed(url("https://example.com"));

        sched.add_urls(
            vec![
                url("https://example.com/page"),
                url("https://other.com/page"),
            ],
            1,
            TaskSource::InternalLink,
        );

        // Only the seed should be in the queue.
        assert_eq!(sched.pending_count(), 1);
    }

    #[test]
    fn test_full_mode_blocks_external() {
        let sched = make_fast_scheduler(ScanMode::Full);
        sched.add_seed(url("https://example.com"));

        sched.add_urls(
            vec![url("https://other.com/external")],
            1,
            TaskSource::ExternalLink,
        );

        // Seed only; external was blocked.
        assert_eq!(sched.pending_count(), 1);
    }

    #[test]
    fn test_full_mode_allows_internal() {
        let sched = make_fast_scheduler(ScanMode::Full);
        sched.add_seed(url("https://example.com"));

        sched.add_urls(
            vec![url("https://example.com/about")],
            1,
            TaskSource::InternalLink,
        );

        // Seed + internal link.
        assert_eq!(sched.pending_count(), 2);
    }

    #[test]
    fn test_deep_mode_allows_external() {
        let sched = make_fast_scheduler(ScanMode::Deep);
        sched.add_seed(url("https://example.com"));

        sched.add_urls(
            vec![url("https://other.com/page")],
            1,
            TaskSource::ExternalLink,
        );

        // Seed + external link.
        assert_eq!(sched.pending_count(), 2);
    }

    #[test]
    fn test_deep_mode_allows_internal() {
        let sched = make_fast_scheduler(ScanMode::Deep);
        sched.add_seed(url("https://example.com"));

        sched.add_urls(
            vec![url("https://example.com/page")],
            1,
            TaskSource::InternalLink,
        );

        // Seed + internal link.
        assert_eq!(sched.pending_count(), 2);
    }

    // ------------------------------------------------------------------
    // Max depth limiting
    // ------------------------------------------------------------------

    #[test]
    fn test_max_depth_limits_urls() {
        let config = CrawlConfig {
            max_pages: 1000,
            max_depth: 2,
            delay_strategy: crate::config::DelayStrategy::Fixed { delay: Duration::ZERO },
            ..CrawlConfig::default()
        };
        let sched = Scheduler::new(&config, ScanMode::Full);
        sched.add_seed(url("https://example.com"));

        // Depth 2 should be accepted (max_depth = 2).
        sched.add_urls(
            vec![url("https://example.com/level2")],
            2,
            TaskSource::InternalLink,
        );
        assert_eq!(sched.pending_count(), 2);

        // Depth 3 should be rejected.
        sched.add_urls(
            vec![url("https://example.com/level3")],
            3,
            TaskSource::InternalLink,
        );
        // Still 2 -- the depth-3 URL was skipped.
        assert_eq!(sched.pending_count(), 2);
    }

    // ------------------------------------------------------------------
    // Max pages limiting
    // ------------------------------------------------------------------

    #[test]
    fn test_max_pages_limits_urls() {
        let config = CrawlConfig {
            max_pages: 3,
            delay_strategy: crate::config::DelayStrategy::Fixed { delay: Duration::ZERO },
            ..CrawlConfig::default()
        };
        let sched = Scheduler::new(&config, ScanMode::Full);
        sched.add_seed(url("https://example.com")); // discovered = 1

        sched.add_urls(
            vec![
                url("https://example.com/a"), // discovered = 2
                url("https://example.com/b"), // discovered = 3
                url("https://example.com/c"), // should be blocked (>= max_pages)
                url("https://example.com/d"), // should be blocked
            ],
            1,
            TaskSource::InternalLink,
        );

        assert_eq!(sched.stats.total_discovered.load(Ordering::Relaxed), 3);
        // Seed + a + b = 3 in queue.
        assert_eq!(sched.pending_count(), 3);
    }

    // ------------------------------------------------------------------
    // Completion detection
    // ------------------------------------------------------------------

    #[test]
    fn test_is_done_initially_empty() {
        let sched = make_fast_scheduler(ScanMode::Full);
        // No seed added yet, so the queue is empty and in_progress is empty.
        assert!(sched.is_done());
    }

    #[test]
    fn test_is_done_with_pending() {
        let sched = make_fast_scheduler(ScanMode::Full);
        sched.add_seed(url("https://example.com"));
        assert!(!sched.is_done());
    }

    #[test]
    fn test_is_done_with_in_progress() {
        let sched = make_fast_scheduler(ScanMode::Full);
        sched.add_seed(url("https://example.com"));

        // Pop the task (now it's in-progress).
        let _task = sched.next_task().unwrap();
        // Queue is empty, but in_progress is not.
        assert!(!sched.is_done());
    }

    #[test]
    fn test_is_done_after_completion() {
        let sched = make_fast_scheduler(ScanMode::Full);
        sched.add_seed(url("https://example.com"));

        let task = sched.next_task().unwrap();
        sched.mark_completed(&task.url);

        assert!(sched.is_done());
    }

    #[test]
    fn test_is_done_after_failure() {
        let sched = make_fast_scheduler(ScanMode::Full);
        sched.add_seed(url("https://example.com"));

        let task = sched.next_task().unwrap();
        sched.mark_failed(&task.url);

        assert!(sched.is_done());
    }

    // ------------------------------------------------------------------
    // Stats tracking
    // ------------------------------------------------------------------

    #[test]
    fn test_stats_tracking() {
        let sched = make_fast_scheduler(ScanMode::Full);
        sched.add_seed(url("https://example.com"));

        sched.add_urls(
            vec![
                url("https://example.com/a"),
                url("https://example.com/b"),
            ],
            1,
            TaskSource::InternalLink,
        );

        let snap = sched.stats.snapshot();
        assert_eq!(snap.total_discovered, 3); // seed + a + b
        assert_eq!(snap.total_crawled, 0);
        assert_eq!(snap.total_errors, 0);

        // Complete one task.
        let task = sched.next_task().unwrap();
        sched.mark_completed(&task.url);

        let snap = sched.stats.snapshot();
        assert_eq!(snap.total_crawled, 1);

        // Fail one task.
        let task = sched.next_task().unwrap();
        sched.mark_failed(&task.url);

        let snap = sched.stats.snapshot();
        assert_eq!(snap.total_crawled, 1);
        assert_eq!(snap.total_errors, 1);
    }

    #[test]
    fn test_skipped_counter() {
        let sched = make_fast_scheduler(ScanMode::Full);
        sched.add_seed(url("https://example.com"));

        // External URL in Full mode -> skipped.
        sched.add_urls(
            vec![url("https://other.com/page")],
            1,
            TaskSource::ExternalLink,
        );

        assert_eq!(sched.stats.total_skipped.load(Ordering::Relaxed), 1);
    }

    // ------------------------------------------------------------------
    // Domain rate limiting
    // ------------------------------------------------------------------

    #[test]
    fn test_domain_rate_limiting() {
        let config = CrawlConfig {
            delay_strategy: crate::config::DelayStrategy::Fixed { delay: Duration::from_secs(10) },
            ..CrawlConfig::default()
        };
        let sched = Scheduler::new(&config, ScanMode::Full);
        sched.add_seed(url("https://example.com"));

        sched.add_urls(
            vec![url("https://example.com/page")],
            1,
            TaskSource::InternalLink,
        );

        // First task should succeed.
        let task = sched.next_task().unwrap();
        sched.mark_completed(&task.url);

        // Second task from the same domain should be rate-limited because we
        // set a 10-second delay and only milliseconds have passed.
        let next = sched.next_task();
        assert!(
            next.is_none(),
            "expected None due to rate limiting, got a task"
        );
    }

    #[test]
    fn test_no_rate_limiting_with_zero_delay() {
        let sched = make_fast_scheduler(ScanMode::Full);
        sched.add_seed(url("https://example.com"));

        sched.add_urls(
            vec![url("https://example.com/page")],
            1,
            TaskSource::InternalLink,
        );

        let task = sched.next_task().unwrap();
        sched.mark_completed(&task.url);

        // With zero delay, the next task should be available immediately.
        let next = sched.next_task();
        assert!(next.is_some(), "expected a task with zero delay");
    }

    // ------------------------------------------------------------------
    // should_crawl
    // ------------------------------------------------------------------

    #[test]
    fn test_should_crawl_new_url() {
        let sched = make_fast_scheduler(ScanMode::Full);
        sched.add_seed(url("https://example.com"));

        assert!(sched.should_crawl(&url("https://example.com/new")));
    }

    #[test]
    fn test_should_crawl_visited_url() {
        let sched = make_fast_scheduler(ScanMode::Full);
        sched.add_seed(url("https://example.com"));

        // The seed itself is visited.
        assert!(!sched.should_crawl(&url("https://example.com")));
    }

    #[test]
    fn test_should_crawl_external_full_mode() {
        let sched = make_fast_scheduler(ScanMode::Full);
        sched.add_seed(url("https://example.com"));

        assert!(!sched.should_crawl(&url("https://other.com/page")));
    }

    #[test]
    fn test_should_crawl_external_deep_mode() {
        let sched = make_fast_scheduler(ScanMode::Deep);
        sched.add_seed(url("https://example.com"));

        assert!(sched.should_crawl(&url("https://other.com/page")));
    }

    #[test]
    fn test_should_crawl_lite_mode() {
        let sched = make_fast_scheduler(ScanMode::Lite);
        sched.add_seed(url("https://example.com"));

        assert!(!sched.should_crawl(&url("https://example.com/page")));
    }

    // ------------------------------------------------------------------
    // set_done
    // ------------------------------------------------------------------

    #[test]
    fn test_set_done() {
        let sched = make_fast_scheduler(ScanMode::Full);
        sched.add_seed(url("https://example.com"));
        assert!(!sched.is_done());

        sched.set_done();
        assert!(sched.is_done());
    }
}
