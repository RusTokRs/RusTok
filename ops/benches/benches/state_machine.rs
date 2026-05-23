use criterion::{criterion_group, criterion_main, Criterion};
use rust_decimal::Decimal;
use rustok_commerce::state_machine::{Order, Pending};
use rustok_content::state_machine::{ContentNode, Draft};
use std::hint::black_box;
use uuid::Uuid;

fn bench_content_state_transitions(c: &mut Criterion) {
    let tenant_id = Uuid::new_v4();
    let author_id = Some(Uuid::new_v4());

    let mut group = c.benchmark_group("content_state_transitions");

    // Benchmark: Draft -> Published transition
    group.bench_function("draft_to_published", |b| {
        b.iter(|| {
            let draft = ContentNode::<Draft>::new_draft(
                Uuid::new_v4(),
                tenant_id,
                author_id,
                "page".to_string(),
            );
            black_box(draft.publish())
        })
    });

    // Benchmark: Published -> Archived transition
    group.bench_function("published_to_archived", |b| {
        b.iter(|| {
            let draft = ContentNode::<Draft>::new_draft(
                Uuid::new_v4(),
                tenant_id,
                author_id,
                "page".to_string(),
            );
            let published = draft.publish();
            black_box(published.archive("Benchmark test".to_string()))
        })
    });

    // Benchmark: Archived -> Draft transition (restore)
    group.bench_function("archived_to_draft", |b| {
        b.iter(|| {
            let draft = ContentNode::<Draft>::new_draft(
                Uuid::new_v4(),
                tenant_id,
                author_id,
                "page".to_string(),
            );
            let published = draft.publish();
            let archived = published.archive("Benchmark test".to_string());
            black_box(archived.restore_to_draft())
        })
    });

    // Benchmark: Full lifecycle Draft -> Published -> Archived
    group.bench_function("full_content_lifecycle", |b| {
        b.iter(|| {
            let draft = ContentNode::<Draft>::new_draft(
                Uuid::new_v4(),
                tenant_id,
                author_id,
                "page".to_string(),
            );
            let published = draft.publish();
            let archived = published.archive("Complete lifecycle".to_string());
            black_box(archived)
        })
    });

    group.finish();
}

fn bench_order_state_transitions(c: &mut Criterion) {
    let tenant_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();

    let mut group = c.benchmark_group("order_state_transitions");

    // Benchmark: Pending -> Confirmed transition
    group.bench_function("pending_to_confirmed", |b| {
        b.iter(|| {
            let pending = Order::<Pending>::new_pending(
                Uuid::new_v4(),
                tenant_id,
                customer_id,
                Decimal::new(10000, 2), // $100.00
                "USD".to_string(),
            );
            black_box(pending.confirm())
        })
    });

    // Benchmark: Confirmed -> Paid transition
    group.bench_function("confirmed_to_paid", |b| {
        b.iter(|| {
            let pending = Order::<Pending>::new_pending(
                Uuid::new_v4(),
                tenant_id,
                customer_id,
                Decimal::new(10000, 2),
                "USD".to_string(),
            );
            let confirmed = pending.confirm().unwrap();
            black_box(confirmed.pay("pay_123".to_string(), "credit_card".to_string()))
        })
    });

    // Benchmark: Full order flow: Pending -> Confirmed -> Paid -> Shipped -> Delivered
    group.bench_function("complete_order_flow", |b| {
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
            let shipped = paid
                .ship("TRACK123".to_string(), "UPS".to_string())
                .unwrap();
            black_box(shipped.deliver(None))
        })
    });

    // Benchmark: Cancellation flow: Pending -> Cancelled
    group.bench_function("pending_to_cancelled", |b| {
        b.iter(|| {
            let pending = Order::<Pending>::new_pending(
                Uuid::new_v4(),
                tenant_id,
                customer_id,
                Decimal::new(10000, 2),
                "USD".to_string(),
            );
            black_box(pending.cancel("Customer request".to_string()))
        })
    });

    group.finish();
}

fn bench_state_clone(c: &mut Criterion) {
    let tenant_id = Uuid::new_v4();
    let author_id = Some(Uuid::new_v4());
    let customer_id = Uuid::new_v4();

    let mut group = c.benchmark_group("state_clone");

    // Benchmark cloning content states
    group.bench_function("clone_content_draft", |b| {
        let draft = ContentNode::<Draft>::new_draft(
            Uuid::new_v4(),
            tenant_id,
            author_id,
            "page".to_string(),
        );
        b.iter(|| black_box(draft.clone()))
    });

    group.bench_function("clone_content_published", |b| {
        let draft = ContentNode::<Draft>::new_draft(
            Uuid::new_v4(),
            tenant_id,
            author_id,
            "page".to_string(),
        );
        let published = draft.publish();
        b.iter(|| black_box(published.clone()))
    });

    // Benchmark cloning order states
    group.bench_function("clone_order_pending", |b| {
        let pending = Order::<Pending>::new_pending(
            Uuid::new_v4(),
            tenant_id,
            customer_id,
            Decimal::new(10000, 2),
            "USD".to_string(),
        );
        b.iter(|| black_box(pending.clone()))
    });

    group.bench_function("clone_order_paid", |b| {
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
        b.iter(|| black_box(paid.clone()))
    });

    group.finish();
}

criterion_group!(
    state_machine_benches,
    bench_content_state_transitions,
    bench_order_state_transitions,
    bench_state_clone
);
criterion_main!(state_machine_benches);
