//! Metrics Collection System
//!
//! Provides standardized metrics collection for RusToK modules.
//!
//! # Features
//!
//! - **Counter Metrics**: For counting events (requests, errors, etc.)
//! - **Gauge Metrics**: For tracking values over time (queue depth, connections)
//! - **Histogram Metrics**: For measuring distributions (latency, sizes)
//! - **Timer Helpers**: For easy latency measurement
//! - **Labels Support**: For dimensional metrics
//!
//! # Example
//!
//! ```rust
//! use rustok_core::metrics::{Counter, Gauge, Histogram, Timer};
//!
//! // Counter
//! let requests = Counter::new("http_requests_total");
//! requests.inc();
//! requests.inc_by(5);
//!
//! // Gauge
//! let connections = Gauge::new("active_connections");
//! connections.set(42);
//! connections.inc();
//!
//! // Histogram
//! let latency = Histogram::new("request_duration_seconds");
//! latency.observe(0.150);
//!
//! // Timer
//! let timer = Timer::start();
//! // ... do work ...
//! latency.observe_timer(timer);
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// A counter metric that only increases
#[derive(Debug, Clone)]
pub struct Counter {
    name: String,
    value: Arc<AtomicU64>,
    labels: HashMap<String, String>,
}

impl Counter {
    /// Create a new counter
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: Arc::new(AtomicU64::new(0)),
            labels: HashMap::new(),
        }
    }

    /// Create a counter with labels
    pub fn with_labels(
        name: impl Into<String>,
        labels: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        Self {
            name: name.into(),
            value: Arc::new(AtomicU64::new(0)),
            labels: labels
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }

    /// Increment by 1
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment by a specific amount
    pub fn inc_by(&self, amount: u64) {
        self.value.fetch_add(amount, Ordering::Relaxed);
    }

    /// Get current value
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Get metric name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get labels
    pub fn labels(&self) -> &HashMap<String, String> {
        &self.labels
    }
}

/// A gauge metric that can go up and down
#[derive(Debug, Clone)]
pub struct Gauge {
    name: String,
    value: Arc<AtomicI64>,
    labels: HashMap<String, String>,
}

impl Gauge {
    /// Create a new gauge
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: Arc::new(AtomicI64::new(0)),
            labels: HashMap::new(),
        }
    }

    /// Create a gauge with labels
    pub fn with_labels(
        name: impl Into<String>,
        labels: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        Self {
            name: name.into(),
            value: Arc::new(AtomicI64::new(0)),
            labels: labels
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }

    /// Set to a specific value
    pub fn set(&self, value: i64) {
        self.value.store(value, Ordering::Relaxed);
    }

    /// Increment by 1
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement by 1
    pub fn dec(&self) {
        self.value.fetch_sub(1, Ordering::Relaxed);
    }

    /// Add a value
    pub fn add(&self, amount: i64) {
        self.value.fetch_add(amount, Ordering::Relaxed);
    }

    /// Subtract a value
    pub fn sub(&self, amount: i64) {
        self.value.fetch_sub(amount, Ordering::Relaxed);
    }

    /// Get current value
    pub fn get(&self) -> i64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Get metric name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get labels
    pub fn labels(&self) -> &HashMap<String, String> {
        &self.labels
    }
}

/// A histogram metric for tracking distributions
#[derive(Debug, Clone)]
pub struct Histogram {
    name: String,
    buckets: Arc<RwLock<Vec<f64>>>,
    sum: Arc<AtomicU64>, // Store as nanoseconds for precision
    count: Arc<AtomicU64>,
    labels: HashMap<String, String>,
}

impl Histogram {
    /// Create a new histogram
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            buckets: Arc::new(RwLock::new(Vec::new())),
            sum: Arc::new(AtomicU64::new(0)),
            count: Arc::new(AtomicU64::new(0)),
            labels: HashMap::new(),
        }
    }

    /// Create a histogram with labels
    pub fn with_labels(
        name: impl Into<String>,
        labels: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        Self {
            name: name.into(),
            buckets: Arc::new(RwLock::new(Vec::new())),
            sum: Arc::new(AtomicU64::new(0)),
            count: Arc::new(AtomicU64::new(0)),
            labels: labels
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }

    /// Observe a value (in seconds)
    pub fn observe(&self, value: f64) {
        // Store as nanoseconds for precision
        let nanos = (value * 1_000_000_000.0) as u64;
        self.sum.fetch_add(nanos, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);

        if let Ok(mut buckets) = self.buckets.write() {
            buckets.push(value);
        }
    }

    /// Observe a timer duration
    pub fn observe_timer(&self, timer: Timer) {
        self.observe(timer.elapsed_secs());
    }

    /// Get the sum of all observations (in seconds)
    pub fn sum(&self) -> f64 {
        let nanos = self.sum.load(Ordering::Relaxed);
        nanos as f64 / 1_000_000_000.0
    }

    /// Get the count of observations
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Calculate average (in seconds)
    pub fn average(&self) -> Option<f64> {
        let count = self.count();
        if count == 0 {
            return None;
        }
        Some(self.sum() / count as f64)
    }

    /// Get percentile (0.0 - 1.0)
    pub fn percentile(&self, p: f64) -> Option<f64> {
        if let Ok(buckets) = self.buckets.read() {
            if buckets.is_empty() {
                return None;
            }

            let mut sorted = buckets.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            let index = ((sorted.len() as f64) * p) as usize;
            sorted.get(index.min(sorted.len() - 1)).copied()
        } else {
            None
        }
    }

    /// Get p50 (median)
    pub fn p50(&self) -> Option<f64> {
        self.percentile(0.5)
    }

    /// Get p95
    pub fn p95(&self) -> Option<f64> {
        self.percentile(0.95)
    }

    /// Get p99
    pub fn p99(&self) -> Option<f64> {
        self.percentile(0.99)
    }

    /// Get metric name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get labels
    pub fn labels(&self) -> &HashMap<String, String> {
        &self.labels
    }
}

/// Timer for measuring durations
#[derive(Debug, Clone)]
pub struct Timer {
    start: Instant,
}

impl Timer {
    /// Start a new timer
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Get elapsed duration
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Get elapsed time in seconds
    pub fn elapsed_secs(&self) -> f64 {
        self.elapsed().as_secs_f64()
    }

    /// Get elapsed time in milliseconds
    pub fn elapsed_millis(&self) -> u64 {
        self.elapsed().as_millis() as u64
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::start()
    }
}

/// Metric snapshot for reporting
#[derive(Debug, Clone)]
pub struct MetricSnapshot {
    pub name: String,
    pub value: MetricValue,
    pub labels: HashMap<String, String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Metric value types
#[derive(Debug, Clone)]
pub enum MetricValue {
    Counter(u64),
    Gauge(i64),
    Histogram {
        count: u64,
        sum: f64,
        p50: Option<f64>,
        p95: Option<f64>,
        p99: Option<f64>,
    },
}

/// Metrics registry for collecting and reporting metrics
#[derive(Debug, Default)]
pub struct MetricsRegistry {
    counters: RwLock<HashMap<String, Counter>>,
    gauges: RwLock<HashMap<String, Gauge>>,
    histograms: RwLock<HashMap<String, Histogram>>,
}

impl MetricsRegistry {
    /// Create a new metrics registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a counter
    pub fn register_counter(&self, counter: Counter) {
        if let Ok(mut counters) = self.counters.write() {
            counters.insert(counter.name.clone(), counter);
        }
    }

    /// Register a gauge
    pub fn register_gauge(&self, gauge: Gauge) {
        if let Ok(mut gauges) = self.gauges.write() {
            gauges.insert(gauge.name.clone(), gauge);
        }
    }

    /// Register a histogram
    pub fn register_histogram(&self, histogram: Histogram) {
        if let Ok(mut histograms) = self.histograms.write() {
            histograms.insert(histogram.name.clone(), histogram);
        }
    }

    /// Get a counter by name
    pub fn counter(&self, name: &str) -> Option<Counter> {
        self.counters.read().ok()?.get(name).cloned()
    }

    /// Get a gauge by name
    pub fn gauge(&self, name: &str) -> Option<Gauge> {
        self.gauges.read().ok()?.get(name).cloned()
    }

    /// Get a histogram by name
    pub fn histogram(&self, name: &str) -> Option<Histogram> {
        self.histograms.read().ok()?.get(name).cloned()
    }

    /// Take a snapshot of all metrics
    pub fn snapshot(&self) -> Vec<MetricSnapshot> {
        let mut snapshots = Vec::new();
        let timestamp = chrono::Utc::now();

        // Counter snapshots
        if let Ok(counters) = self.counters.read() {
            for counter in counters.values() {
                snapshots.push(MetricSnapshot {
                    name: counter.name.clone(),
                    value: MetricValue::Counter(counter.get()),
                    labels: counter.labels.clone(),
                    timestamp,
                });
            }
        }

        // Gauge snapshots
        if let Ok(gauges) = self.gauges.read() {
            for gauge in gauges.values() {
                snapshots.push(MetricSnapshot {
                    name: gauge.name.clone(),
                    value: MetricValue::Gauge(gauge.get()),
                    labels: gauge.labels.clone(),
                    timestamp,
                });
            }
        }

        // Histogram snapshots
        if let Ok(histograms) = self.histograms.read() {
            for histogram in histograms.values() {
                snapshots.push(MetricSnapshot {
                    name: histogram.name.clone(),
                    value: MetricValue::Histogram {
                        count: histogram.count(),
                        sum: histogram.sum(),
                        p50: histogram.p50(),
                        p95: histogram.p95(),
                        p99: histogram.p99(),
                    },
                    labels: histogram.labels.clone(),
                    timestamp,
                });
            }
        }

        snapshots
    }

    /// Export metrics as Prometheus format
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();

        // Counters
        if let Ok(counters) = self.counters.read() {
            for counter in counters.values() {
                output.push_str(&format!("# TYPE {} counter\n", counter.name));
                output.push_str(&format_counter(counter));
            }
        }

        // Gauges
        if let Ok(gauges) = self.gauges.read() {
            for gauge in gauges.values() {
                output.push_str(&format!("# TYPE {} gauge\n", gauge.name));
                output.push_str(&format_gauge(gauge));
            }
        }

        // Histograms
        if let Ok(histograms) = self.histograms.read() {
            for histogram in histograms.values() {
                output.push_str(&format!("# TYPE {} histogram\n", histogram.name));
                output.push_str(&format_histogram(histogram));
            }
        }

        output
    }
}

fn format_labels(labels: &HashMap<String, String>) -> String {
    if labels.is_empty() {
        return String::new();
    }

    let label_str: Vec<String> = labels
        .iter()
        .map(|(k, v)| format!("{}=\"{}\"", k, v))
        .collect();

    format!("{{{}}}", label_str.join(","))
}

fn format_counter(counter: &Counter) -> String {
    format!(
        "{}{} {}\n",
        counter.name,
        format_labels(&counter.labels),
        counter.get()
    )
}

fn format_gauge(gauge: &Gauge) -> String {
    format!(
        "{}{} {}\n",
        gauge.name,
        format_labels(&gauge.labels),
        gauge.get()
    )
}

fn format_histogram(histogram: &Histogram) -> String {
    let mut output = String::new();
    let name = &histogram.name;
    let labels = format_labels(&histogram.labels);

    // Count
    output.push_str(&format!("{}_count{} {}\n", name, labels, histogram.count()));

    // Sum
    output.push_str(&format!("{}_sum{} {}\n", name, labels, histogram.sum()));

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter() {
        let counter = Counter::new("test_counter");
        counter.inc();
        counter.inc_by(5);
        assert_eq!(counter.get(), 6);
    }

    #[test]
    fn test_gauge() {
        let gauge = Gauge::new("test_gauge");
        gauge.set(10);
        gauge.inc();
        gauge.add(5);
        assert_eq!(gauge.get(), 16);
        gauge.sub(6);
        assert_eq!(gauge.get(), 10);
    }

    #[test]
    fn test_histogram() {
        let histogram = Histogram::new("test_histogram");
        histogram.observe(0.1);
        histogram.observe(0.2);
        histogram.observe(0.3);

        assert_eq!(histogram.count(), 3);
        assert!(histogram.sum() > 0.59 && histogram.sum() < 0.61);

        let p50 = histogram.p50().unwrap();
        assert!((0.1..=0.3).contains(&p50));
    }

    #[test]
    fn test_timer() {
        let timer = Timer::start();
        std::thread::sleep(Duration::from_millis(10));
        assert!(timer.elapsed_millis() >= 10);
    }

    #[test]
    fn test_metrics_registry() {
        let registry = MetricsRegistry::new();

        let counter = Counter::new("requests");
        counter.inc_by(100);
        registry.register_counter(counter);

        let gauge = Gauge::new("connections");
        gauge.set(42);
        registry.register_gauge(gauge);

        let snapshot = registry.snapshot();
        assert_eq!(snapshot.len(), 2);
    }

    #[test]
    fn test_prometheus_export() {
        let registry = MetricsRegistry::new();

        let counter = Counter::with_labels("http_requests", [("method", "GET"), ("status", "200")]);
        counter.inc_by(10);
        registry.register_counter(counter);

        let output = registry.export_prometheus();
        assert!(output.contains("# TYPE http_requests counter"));
        assert!(output.contains("http_requests{"));
        assert!(output.contains("method=\"GET\""));
        assert!(output.contains("10"));
    }

    #[test]
    fn test_counter_with_labels() {
        let counter =
            Counter::with_labels("api_requests", [("endpoint", "/users"), ("method", "GET")]);

        assert_eq!(counter.name(), "api_requests");
        assert_eq!(
            counter.labels().get("endpoint"),
            Some(&"/users".to_string())
        );
    }
}
