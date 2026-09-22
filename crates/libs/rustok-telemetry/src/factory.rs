use prometheus::{
    CounterVec, GaugeVec, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGauge,
    IntGaugeVec, Opts,
};

pub(crate) fn create_counter_vec(name: &str, help: &str, label_names: &[&str]) -> CounterVec {
    // INVARIANT: Static Prometheus metric configuration with constant name and labels.
    CounterVec::new(Opts::new(name, help), label_names)
        .expect("static Prometheus counter vector must configure cleanly")
}

pub(crate) fn create_int_counter(name: &str, help: &str) -> IntCounter {
    // INVARIANT: Static Prometheus metric configuration with constant name.
    IntCounter::new(name, help)
        .expect("static Prometheus integer counter must configure cleanly")
}

pub(crate) fn create_int_counter_vec(
    name: &str,
    help: &str,
    label_names: &[&str],
) -> IntCounterVec {
    // INVARIANT: Static Prometheus metric configuration with constant name and labels.
    IntCounterVec::new(Opts::new(name, help), label_names)
        .expect("static Prometheus integer counter vector must configure cleanly")
}

pub(crate) fn create_int_gauge(name: &str, help: &str) -> IntGauge {
    // INVARIANT: Static Prometheus metric configuration with constant name.
    IntGauge::new(name, help)
        .expect("static Prometheus integer gauge must configure cleanly")
}

pub(crate) fn create_int_gauge_vec(name: &str, help: &str, label_names: &[&str]) -> IntGaugeVec {
    // INVARIANT: Static Prometheus metric configuration with constant name and labels.
    IntGaugeVec::new(Opts::new(name, help), label_names)
        .expect("static Prometheus integer gauge vector must configure cleanly")
}

pub(crate) fn create_gauge_vec(name: &str, help: &str, label_names: &[&str]) -> GaugeVec {
    // INVARIANT: Static Prometheus metric configuration with constant name and labels.
    GaugeVec::new(Opts::new(name, help), label_names)
        .expect("static Prometheus gauge vector must configure cleanly")
}

pub(crate) fn create_histogram_vec(
    name: &str,
    help: &str,
    buckets: Vec<f64>,
    label_names: &[&str],
) -> HistogramVec {
    // INVARIANT: Static Prometheus metric configuration with constant name, buckets, and labels.
    HistogramVec::new(HistogramOpts::new(name, help).buckets(buckets), label_names)
        .expect("static Prometheus histogram vector must configure cleanly")
}
