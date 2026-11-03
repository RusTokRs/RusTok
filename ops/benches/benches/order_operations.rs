use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rust_decimal::Decimal;
use rustok_commerce::state_machine::{Order, Paid, Pending};
use uuid::Uuid;

/// Benchmark order state machine workflows
fn bench_order_workflows(c: &mut Criterion) {
    let tenant_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();

    let mut group = c.benchmark_group("order_workflow");

    // Complete order flow: Pending -> Confirmed -> Paid -> Shipped -> Delivered
    group.bench_function("complete_order_flow", |b| {
        b.iter(|| {
            let pending = Order::<Pending>::new_pending(
                Uuid::new_v4(),
                tenant_id,
                customer_id,
                Decimal::new(10000, 2), // $100.00
                "USD".to_string(),
            );
            let confirmed = pending.confirm().unwrap();
            let paid = confirmed
                .pay("pay_123".to_string(), "credit_card".to_string())
                .unwrap();
            let shipped = paid
                .ship("TRACK123".to_string(), "UPS".to_string())
                .unwrap();
            let delivered = shipped.deliver(None);
            std::hint::black_box(delivered)
        })
    });

    // Cancellation flow: Pending -> Cancelled
    group.bench_function("cancellation_flow", |b| {
        b.iter(|| {
            let pending = Order::<Pending>::new_pending(
                Uuid::new_v4(),
                tenant_id,
                customer_id,
                Decimal::new(10000, 2),
                "USD".to_string(),
            );
            let cancelled = pending.cancel("Customer request".to_string());
            std::hint::black_box(cancelled)
        })
    });

    // Quick flow: Pending -> Confirmed -> Paid
    group.bench_function("payment_flow", |b| {
        b.iter(|| {
            let pending = Order::<Pending>::new_pending(
                Uuid::new_v4(),
                tenant_id,
                customer_id,
                Decimal::new(10000, 2),
                "USD".to_string(),
            );
            let confirmed = pending.confirm().unwrap();
            let paid = confirmed
                .pay("pay_123".to_string(), "credit_card".to_string())
                .unwrap();
            std::hint::black_box(paid)
        })
    });

    group.finish();
}

/// Benchmark order queries and validations
fn bench_order_queries(c: &mut Criterion) {
    let tenant_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();

    let mut group = c.benchmark_group("order_queries");

    group.bench_function("get_id_pending", |b| {
        let pending = Order::<Pending>::new_pending(
            Uuid::new_v4(),
            tenant_id,
            customer_id,
            Decimal::new(10000, 2),
            "USD".to_string(),
        );
        b.iter(|| std::hint::black_box(pending.id))
    });

    group.bench_function("get_total_pending", |b| {
        let pending = Order::<Pending>::new_pending(
            Uuid::new_v4(),
            tenant_id,
            customer_id,
            Decimal::new(10000, 2),
            "USD".to_string(),
        );
        b.iter(|| std::hint::black_box(pending.total_amount))
    });

    group.bench_function("get_total_paid", |b| {
        let pending = Order::<Pending>::new_pending(
            Uuid::new_v4(),
            tenant_id,
            customer_id,
            Decimal::new(10000, 2),
            "USD".to_string(),
        );
        let confirmed = pending.confirm().unwrap();
        let paid = confirmed
            .pay("pay_123".to_string(), "credit_card".to_string())
            .unwrap();
        b.iter(|| std::hint::black_box(paid.total_amount))
    });

    group.finish();
}

/// Benchmark high-volume order processing
fn bench_order_throughput(c: &mut Criterion) {
    let tenant_id = Uuid::new_v4();

    let mut group = c.benchmark_group("order_throughput");

    for batch_size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let orders: Vec<Order<Paid>> = (0..size)
                        .map(|i| {
                            let customer_id = Uuid::new_v4();
                            let pending = Order::<Pending>::new_pending(
                                Uuid::new_v4(),
                                tenant_id,
                                customer_id,
                                Decimal::new((i as i64) * 100, 2),
                                "USD".to_string(),
                            );
                            let confirmed = pending.confirm().unwrap();
                            confirmed
                                .pay("pay_123".to_string(), "credit_card".to_string())
                                .unwrap()
                        })
                        .collect();
                    std::hint::black_box(orders.len())
                })
            },
        );
    }

    group.finish();
}

/// Benchmark monetary calculations
fn bench_order_monetary(c: &mut Criterion) {
    let tenant_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();

    let mut group = c.benchmark_group("order_monetary");

    group.bench_function("checkout_with_large_amount", |b| {
        b.iter(|| {
            let pending = Order::<Pending>::new_pending(
                Uuid::new_v4(),
                tenant_id,
                customer_id,
                Decimal::new(1_000_000_000, 2), // $10M
                "USD".to_string(),
            );
            std::hint::black_box(pending)
        })
    });

    group.bench_function("checkout_with_small_amount", |b| {
        b.iter(|| {
            let pending = Order::<Pending>::new_pending(
                Uuid::new_v4(),
                tenant_id,
                customer_id,
                Decimal::new(1, 2), // $0.01
                "USD".to_string(),
            );
            std::hint::black_box(pending)
        })
    });

    group.bench_function("checkout_with_zero_amount", |b| {
        b.iter(|| {
            let pending = Order::<Pending>::new_pending(
                Uuid::new_v4(),
                tenant_id,
                customer_id,
                Decimal::new(0, 2), // $0.00
                "USD".to_string(),
            );
            std::hint::black_box(pending)
        })
    });

    group.finish();
}

/// Benchmark concurrent order operations
fn bench_order_concurrent(c: &mut Criterion) {
    use std::thread;

    let tenant_id = Uuid::new_v4();

    let mut group = c.benchmark_group("order_concurrent");

    group.bench_function("parallel_checkout", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    thread::spawn(move || {
                        for i in 0..25 {
                            let customer_id = Uuid::new_v4();
                            let pending = Order::<Pending>::new_pending(
                                Uuid::new_v4(),
                                tenant_id,
                                customer_id,
                                Decimal::new((i as i64) * 100, 2),
                                "USD".to_string(),
                            );
                            let confirmed = pending.confirm().unwrap();
                            let paid = confirmed
                                .pay("pay_123".to_string(), "credit_card".to_string())
                                .unwrap();
                            std::hint::black_box(paid);
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        })
    });

    group.finish();
}

criterion_group!(
    order_operations_benches,
    bench_order_workflows,
    bench_order_queries,
    bench_order_throughput,
    bench_order_monetary,
    bench_order_concurrent
);
criterion_main!(order_operations_benches);
