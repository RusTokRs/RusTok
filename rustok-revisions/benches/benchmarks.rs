//! Benchmarks for rustok-revisions.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use rustok_revisions::{
    ChangeSource, Revisionable, RevisionConfig, RevisionEvent, RevisionService, RevisionTracker,
    RetentionPolicy, SeaOrmBackend,
};
use sea_orm::{Database, DatabaseConnection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, Revisionable)]
#[revision(content_type = "benchmark_post")]
struct BenchmarkPost {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub author: String,
    pub tags: Vec<String>,
    pub views: i32,
}

impl RevisionConfig for BenchmarkPost {
    fn default_retention_policy() -> Option<RetentionPolicy> {
        Some(RetentionPolicy::KeepLast(1000))
    }
}

async fn setup_service() -> RevisionService {
    let database_url = std::env::var("BENCH_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/rustok_revisions_bench".to_string());
    
    let db: DatabaseConnection = Database::connect(&database_url)
        .await
        .expect("Failed to connect to database");
    
    let backend = SeaOrmBackend::new(db);
    RevisionService::new(Box::new(backend))
}

fn create_test_post(index: i32) -> BenchmarkPost {
    BenchmarkPost {
        id: Uuid::new_v4(),
        title: format!("Benchmark Post {}", index),
        content: format!("This is benchmark content number {}. ", index).repeat(100),
        author: "Benchmark Author".to_string(),
        tags: vec!["benchmark".to_string(), "test".to_string(), format!("tag-{}", index)],
        views: index * 10,
    }
}

fn bench_create_revision(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let service = rt.block_on(setup_service());
    
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let post = create_test_post(1);

    c.bench_function("create_revision", |b| {
        b.to_async(&rt).iter(|| async {
            service
                .create_revision_for_create(
                    black_box(tenant_id),
                    black_box(post.id),
                    black_box("en"),
                    black_box(&post),
                    black_box(user_id),
                    black_box(ChangeSource::Web),
                )
                .await
                .unwrap()
        })
    });
}

fn bench_update_revision(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let service = rt.block_on(setup_service());
    
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let post = create_test_post(2);
    
    // Create initial revision
    rt.block_on(async {
        service
            .create_revision_for_create(tenant_id, post.id, "en", &post, user_id, ChangeSource::Web)
            .await
            .unwrap()
    });

    let tracker = RevisionTracker::builder()
        .enabled(true)
        .source(ChangeSource::Web)
        .build();

    c.bench_function("update_revision", |b| {
        b.to_async(&rt).iter(|| async {
            let old_post = post.clone();
            let mut new_post = post.clone();
            new_post.views += 1;
            
            service
                .create_revision_with_tracker(
                    black_box(tenant_id),
                    black_box(post.id),
                    black_box("en"),
                    black_box(&old_post),
                    black_box(&new_post),
                    black_box(user_id),
                    black_box(&tracker),
                    black_box(RevisionEvent::Update),
                )
                .await
                .unwrap()
        })
    });
}

fn bench_list_revisions(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let service = rt.block_on(setup_service());
    
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let post = create_test_post(3);
    
    // Create 100 revisions
    rt.block_on(async {
        service
            .create_revision_for_create(tenant_id, post.id, "en", &post, user_id, ChangeSource::Web)
            .await
            .unwrap();
        
        let tracker = RevisionTracker::builder().enabled(true).build();
        
        for i in 0..99 {
            let old_post = post.clone();
            let mut new_post = post.clone();
            new_post.views = i;
            
            service
                .create_revision_with_tracker(
                    tenant_id,
                    post.id,
                    "en",
                    &old_post,
                    &new_post,
                    user_id,
                    &tracker,
                    RevisionEvent::Update,
                )
                .await
                .unwrap();
        }
    });

    c.bench_function("list_revisions_100", |b| {
        b.to_async(&rt).iter(|| async {
            service
                .list_revisions(
                    black_box(tenant_id),
                    black_box(post.id),
                    black_box("en"),
                    black_box(None),
                    black_box(None),
                )
                .await
                .unwrap()
        })
    });

    c.bench_function("list_revisions_10", |b| {
        b.to_async(&rt).iter(|| async {
            service
                .list_revisions(
                    black_box(tenant_id),
                    black_box(post.id),
                    black_box("en"),
                    black_box(Some(10)),
                    black_box(None),
                )
                .await
                .unwrap()
        })
    });
}

fn bench_compare_revisions(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let service = rt.block_on(setup_service());
    
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let post = create_test_post(4);
    
    // Create two revisions
    let (rev1, rev2) = rt.block_on(async {
        let rev1 = service
            .create_revision_for_create(tenant_id, post.id, "en", &post, user_id, ChangeSource::Web)
            .await
            .unwrap();
        
        let mut updated_post = post.clone();
        updated_post.title = "Updated Title".to_string();
        updated_post.content = "Updated content".repeat(100);
        updated_post.views = 999;
        
        let tracker = RevisionTracker::builder().enabled(true).build();
        
        let rev2 = service
            .create_revision_with_tracker(
                tenant_id,
                post.id,
                "en",
                &post,
                &updated_post,
                user_id,
                &tracker,
                RevisionEvent::Update,
            )
            .await
            .unwrap()
            .unwrap();
        
        (rev1, rev2)
    });

    c.bench_function("compare_revisions", |b| {
        b.to_async(&rt).iter(|| async {
            service
                .compare_revisions(black_box(rev1.id), black_box(rev2.id))
                .await
                .unwrap()
        })
    });
}

fn bench_batch_operations(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let service = rt.block_on(setup_service());
    
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    let mut group = c.benchmark_group("batch_operations");
    
    for size in [10, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let posts: Vec<_> = (0..size).map(|i| create_test_post(i as i32)).collect();
                
                for post in posts {
                    service
                        .create_revision_for_create(
                            black_box(tenant_id),
                            black_box(post.id),
                            black_box("en"),
                            black_box(&post),
                            black_box(user_id),
                            black_box(ChangeSource::Web),
                        )
                        .await
                        .unwrap();
                }
            })
        });
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_create_revision,
    bench_update_revision,
    bench_list_revisions,
    bench_compare_revisions,
    bench_batch_operations,
);

criterion_main!(benches);
