use std::time::Instant;

#[derive(Clone, Copy, Debug)]
pub struct EventConsumerRuntime {
    consumer: &'static str,
}

impl EventConsumerRuntime {
    pub const fn new(consumer: &'static str) -> Self {
        Self { consumer }
    }

    pub const fn consumer(&self) -> &'static str {
        self.consumer
    }

    pub fn restarted(&self, reason: &'static str) {
        rustok_telemetry::metrics::record_event_consumer_restarted(self.consumer, reason);
        tracing::info!(
            consumer = self.consumer,
            reason,
            "Event consumer loop started"
        );
    }

    pub fn lagged(&self, skipped: u64) {
        rustok_telemetry::metrics::record_event_consumer_lagged(self.consumer);
        tracing::warn!(
            consumer = self.consumer,
            skipped,
            "Event consumer lagged and skipped messages"
        );
    }

    pub fn closed(&self) {
        tracing::info!(consumer = self.consumer, "Event consumer stream closed");
    }

    pub fn record_dispatch_latency(&self, event_type: &str, started_at: Instant) {
        rustok_telemetry::metrics::record_event_dispatch_latency_ms(
            self.consumer,
            event_type,
            started_at.elapsed().as_secs_f64() * 1000.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::EventConsumerRuntime;

    #[test]
    fn event_consumer_runtime_methods_do_not_panic() {
        let runtime = EventConsumerRuntime::new("test_consumer");

        runtime.restarted("startup");
        runtime.lagged(3);
        runtime.closed();
        runtime.record_dispatch_latency("test.event", std::time::Instant::now());
    }
}
