use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rustok_content::state_machine::{Archived, ContentNode, Draft, Published};
use std::hint::black_box;
use uuid::Uuid;

/// Benchmark content workflow transitions
fn bench_content_workflow(c: &mut Criterion) {
    let tenant_id = Uuid::new_v4();
    let author_id = Some(Uuid::new_v4());

    let mut group = c.benchmark_group("content_workflow");

    // Full workflow: Draft -> Published -> Archived
    group.bench_function("full_publish_workflow", |b| {
        b.iter(|| {
            let draft = ContentNode::<Draft>::new_draft(
                Uuid::new_v4(),
                tenant_id,
                author_id,
                "page".to_string(),
            );
            let published: ContentNode<Published> = draft.publish();
            let archived: ContentNode<Archived> =
                published.archive("Published workflow".to_string());
            black_box(archived)
        })
    });

    // Restore workflow: Draft -> Published -> Archived -> Draft
    group.bench_function("restore_workflow", |b| {
        b.iter(|| {
            let draft = ContentNode::<Draft>::new_draft(
                Uuid::new_v4(),
                tenant_id,
                author_id,
                "page".to_string(),
            );
            let published = draft.publish();
            let archived = published.archive("Restore test".to_string());
            let restored: ContentNode<Draft> = archived.restore_to_draft();
            black_box(restored)
        })
    });

    // Archive workflow: Draft -> Published -> Archived
    group.bench_function("archive_workflow", |b| {
        b.iter(|| {
            let draft = ContentNode::<Draft>::new_draft(
                Uuid::new_v4(),
                tenant_id,
                author_id,
                "page".to_string(),
            );
            let published = draft.publish();
            let archived = published.archive("Archive workflow".to_string());
            black_box(archived)
        })
    });

    group.finish();
}

/// Benchmark content queries and accessors
fn bench_content_queries(c: &mut Criterion) {
    let tenant_id = Uuid::new_v4();
    let author_id = Some(Uuid::new_v4());

    let mut group = c.benchmark_group("content_queries");

    group.bench_function("get_id", |b| {
        let draft = ContentNode::<Draft>::new_draft(
            Uuid::new_v4(),
            tenant_id,
            author_id,
            "page".to_string(),
        );
        b.iter(|| black_box(draft.id()))
    });

    group.bench_function("get_tenant_id", |b| {
        let draft = ContentNode::<Draft>::new_draft(
            Uuid::new_v4(),
            tenant_id,
            author_id,
            "page".to_string(),
        );
        b.iter(|| black_box(draft.tenant_id()))
    });

    group.bench_function("set_parent", |b| {
        let draft = ContentNode::<Draft>::new_draft(
            Uuid::new_v4(),
            tenant_id,
            author_id,
            "page".to_string(),
        );
        let parent_id = Uuid::new_v4();
        b.iter(|| black_box(draft.clone().set_parent(parent_id)))
    });

    group.bench_function("set_category", |b| {
        let draft = ContentNode::<Draft>::new_draft(
            Uuid::new_v4(),
            tenant_id,
            author_id,
            "page".to_string(),
        );
        let category_id = Uuid::new_v4();
        b.iter(|| black_box(draft.clone().set_category(category_id)))
    });

    group.finish();
}

/// Benchmark batch operations
fn bench_content_batch(c: &mut Criterion) {
    let tenant_id = Uuid::new_v4();
    let author_id = Some(Uuid::new_v4());

    let mut group = c.benchmark_group("content_batch");

    for batch_size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let nodes: Vec<ContentNode<Published>> = (0..size)
                        .map(|_| {
                            let draft = ContentNode::<Draft>::new_draft(
                                Uuid::new_v4(),
                                tenant_id,
                                author_id,
                                "page".to_string(),
                            );
                            draft.publish()
                        })
                        .collect();
                    black_box(nodes.len())
                })
            },
        );
    }

    group.finish();
}

/// Benchmark state serialization
fn bench_content_serialization(c: &mut Criterion) {
    let tenant_id = Uuid::new_v4();
    let author_id = Some(Uuid::new_v4());

    let mut group = c.benchmark_group("content_serialization");

    group.bench_function("serialize_draft", |b| {
        let draft = ContentNode::<Draft>::new_draft(
            Uuid::new_v4(),
            tenant_id,
            author_id,
            "page".to_string(),
        );
        b.iter(|| {
            let json = serde_json::to_string(&draft).unwrap();
            black_box(json.len())
        })
    });

    group.bench_function("serialize_published", |b| {
        let draft = ContentNode::<Draft>::new_draft(
            Uuid::new_v4(),
            tenant_id,
            author_id,
            "page".to_string(),
        );
        let published = draft.publish();
        b.iter(|| {
            let json = serde_json::to_string(&published).unwrap();
            black_box(json.len())
        })
    });

    group.bench_function("deserialize_draft", |b| {
        let draft = ContentNode::<Draft>::new_draft(
            Uuid::new_v4(),
            tenant_id,
            author_id,
            "page".to_string(),
        );
        let json = serde_json::to_string(&draft).unwrap();

        b.iter(|| {
            let deserialized: ContentNode<Draft> = serde_json::from_str(&json).unwrap();
            black_box(deserialized)
        })
    });

    group.bench_function("deserialize_published", |b| {
        let draft = ContentNode::<Draft>::new_draft(
            Uuid::new_v4(),
            tenant_id,
            author_id,
            "page".to_string(),
        );
        let published = draft.publish();
        let json = serde_json::to_string(&published).unwrap();

        b.iter(|| {
            let deserialized: ContentNode<Published> = serde_json::from_str(&json).unwrap();
            black_box(deserialized)
        })
    });

    group.finish();
}

criterion_group!(
    content_operations_benches,
    bench_content_workflow,
    bench_content_queries,
    bench_content_batch,
    bench_content_serialization
);
criterion_main!(content_operations_benches);
