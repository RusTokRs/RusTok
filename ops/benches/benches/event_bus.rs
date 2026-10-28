use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;
use std::sync::Arc;
use uuid::Uuid;

// Type aliases for benchmarking
type TenantId = Uuid;
type EventId = Uuid;

/// Simplified event types for benchmarking
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct DomainEvent {
    id: EventId,
    tenant_id: TenantId,
    event_type: String,
    payload: Vec<u8>,
}

impl DomainEvent {
    fn new(tenant_id: TenantId, event_type: &str, payload_size: usize) -> Self {
        Self {
            id: EventId::new_v4(),
            tenant_id,
            event_type: event_type.to_string(),
            payload: vec![0u8; payload_size],
        }
    }
}

/// Simple event bus implementation for benchmarking
mod event_bus_sim {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    type SubscriberList = Mutex<Vec<Box<dyn Fn(&DomainEvent) + Send>>>;

    pub struct EventBus {
        queue: Mutex<VecDeque<DomainEvent>>,
        subscribers: SubscriberList,
    }

    impl EventBus {
        pub fn new() -> Self {
            Self {
                queue: Mutex::new(VecDeque::new()),
                subscribers: Mutex::new(Vec::new()),
            }
        }

        pub fn publish(&self, event: DomainEvent) {
            self.queue.lock().unwrap().push_back(event);
        }

        pub fn publish_immediate(&self, event: DomainEvent) {
            let subscribers = self.subscribers.lock().unwrap();
            for subscriber in subscribers.iter() {
                subscriber(&event);
            }
        }

        pub fn subscribe<F>(&self, handler: F)
        where
            F: Fn(&DomainEvent) + Send + 'static,
        {
            self.subscribers.lock().unwrap().push(Box::new(handler));
        }

        #[allow(dead_code)]
        pub fn drain(&self) -> Vec<DomainEvent> {
            self.queue.lock().unwrap().drain(..).collect()
        }

        pub fn len(&self) -> usize {
            self.queue.lock().unwrap().len()
        }
    }
}

use event_bus_sim::*;

fn bench_event_publishing(c: &mut Criterion) {
    let tenant_id = TenantId::new_v4();

    let mut group = c.benchmark_group("event_publish");

    // Benchmark: Publish small events
    group.bench_function("small_event", |b| {
        let bus = EventBus::new();
        b.iter(|| {
            let event = DomainEvent::new(tenant_id, "test", 100);
            bus.publish(black_box(event));
        })
    });

    // Benchmark: Publish medium events
    group.bench_function("medium_event", |b| {
        let bus = EventBus::new();
        b.iter(|| {
            let event = DomainEvent::new(tenant_id, "test", 1024);
            bus.publish(black_box(event));
        })
    });

    // Benchmark: Publish large events
    group.bench_function("large_event", |b| {
        let bus = EventBus::new();
        b.iter(|| {
            let event = DomainEvent::new(tenant_id, "test", 65536);
            bus.publish(black_box(event));
        })
    });

    group.finish();
}

fn bench_event_throughput(c: &mut Criterion) {
    let tenant_id = TenantId::new_v4();

    let mut group = c.benchmark_group("event_throughput");

    for event_count in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*event_count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(event_count),
            event_count,
            |b, &count| {
                let bus = EventBus::new();

                b.iter(|| {
                    for i in 0..count {
                        let event = DomainEvent::new(tenant_id, &format!("event-{}", i), 256);
                        bus.publish(event);
                    }
                    black_box(bus.len())
                })
            },
        );
    }

    group.finish();
}

fn bench_event_delivery(c: &mut Criterion) {
    let tenant_id = TenantId::new_v4();

    let mut group = c.benchmark_group("event_delivery");

    // Benchmark: Single subscriber
    group.bench_function("single_subscriber", |b| {
        let bus = EventBus::new();
        let received = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        {
            let received = received.clone();
            bus.subscribe(move |_event| {
                received.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
        }

        b.iter(|| {
            let event = DomainEvent::new(tenant_id, "test", 256);
            bus.publish_immediate(event);
        });

        black_box(received.load(std::sync::atomic::Ordering::SeqCst));
    });

    // Benchmark: Multiple subscribers
    group.bench_function("multiple_subscribers", |b| {
        let bus = EventBus::new();
        let subscriber_count = 10;
        let counters: Vec<_> = (0..subscriber_count)
            .map(|_| Arc::new(std::sync::atomic::AtomicUsize::new(0)))
            .collect();

        for counter in &counters {
            let counter = counter.clone();
            bus.subscribe(move |_event| {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
        }

        b.iter(|| {
            let event = DomainEvent::new(tenant_id, "test", 256);
            bus.publish_immediate(event);
        });

        let total: usize = counters
            .iter()
            .map(|c| c.load(std::sync::atomic::Ordering::SeqCst))
            .sum();
        black_box(total);
    });

    group.finish();
}

fn bench_event_filtering(c: &mut Criterion) {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let tenant_id = TenantId::new_v4();
    let other_tenant = TenantId::new_v4();

    let mut group = c.benchmark_group("event_filtering");

    group.bench_function("tenant_filter", |b| {
        let bus = EventBus::new();
        let matched = Arc::new(AtomicUsize::new(0));

        {
            let matched = matched.clone();
            bus.subscribe(move |event| {
                if event.tenant_id == tenant_id {
                    matched.fetch_add(1, Ordering::SeqCst);
                }
            });
        }

        let mut toggle = true;
        b.iter(|| {
            // Alternate between target tenant and other tenant
            let id = if toggle { tenant_id } else { other_tenant };
            toggle = !toggle;

            let event = DomainEvent::new(id, "test", 256);
            bus.publish_immediate(event);
        });

        black_box(matched.load(Ordering::SeqCst));
    });

    group.finish();
}

criterion_group!(
    event_bus_benches,
    bench_event_publishing,
    bench_event_throughput,
    bench_event_delivery,
    bench_event_filtering
);
criterion_main!(event_bus_benches);
