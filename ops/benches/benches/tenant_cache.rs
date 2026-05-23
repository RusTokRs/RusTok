use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;
use std::hint::black_box;
use std::sync::Arc;
use uuid::Uuid;

// TenantId is a type alias for Uuid
type TenantId = Uuid;

/// Simulate tenant cache operations
mod tenant_cache_sim {
    use super::*;
    use std::sync::RwLock;

    #[derive(Clone)]
    pub struct TenantData {
        pub id: TenantId,
        pub name: String,
        pub config: HashMap<String, String>,
    }

    pub struct TenantCache {
        cache: RwLock<HashMap<TenantId, Arc<TenantData>>>,
    }

    impl TenantCache {
        pub fn new() -> Self {
            Self {
                cache: RwLock::new(HashMap::new()),
            }
        }

        pub fn get(&self, id: &TenantId) -> Option<Arc<TenantData>> {
            self.cache.read().unwrap().get(id).cloned()
        }

        pub fn insert(&self, data: TenantData) {
            self.cache.write().unwrap().insert(data.id, Arc::new(data));
        }

        pub fn invalidate(&self, id: &TenantId) {
            self.cache.write().unwrap().remove(id);
        }

        pub fn len(&self) -> usize {
            self.cache.read().unwrap().len()
        }
    }
}

use tenant_cache_sim::*;

fn bench_cache_operations(c: &mut Criterion) {
    let cache = TenantCache::new();
    let tenant_ids: Vec<TenantId> = (0..1000).map(|_| TenantId::new_v4()).collect();

    // Pre-populate cache
    for (i, id) in tenant_ids.iter().enumerate() {
        let data = TenantData {
            id: *id,
            name: format!("tenant-{}", i),
            config: [
                ("key1".to_string(), "value1".to_string()),
                ("key2".to_string(), "value2".to_string()),
                ("key3".to_string(), "value3".to_string()),
            ]
            .into_iter()
            .collect(),
        };
        cache.insert(data);
    }

    black_box(cache.len());

    let mut group = c.benchmark_group("tenant_cache");

    // Benchmark: Cache hit (read-only)
    group.bench_function("get_hit", |b| {
        let id = tenant_ids[500];
        b.iter(|| {
            let result = cache
                .get(&id)
                .map(|tenant| (tenant.name.len(), tenant.config.len()));
            black_box(result)
        })
    });

    // Benchmark: Cache miss
    group.bench_function("get_miss", |b| {
        let missing_id = TenantId::new_v4();
        b.iter(|| {
            let result = cache
                .get(&missing_id)
                .map(|tenant| (tenant.name.len(), tenant.config.len()));
            black_box(result)
        })
    });

    // Benchmark: Insert (write)
    group.bench_function("insert", |b| {
        let mut counter = 0u64;
        b.iter(|| {
            counter += 1;
            let id = TenantId::new_v4();
            let data = TenantData {
                id,
                name: format!("new-tenant-{}", counter),
                config: HashMap::new(),
            };
            cache.insert(data);
            black_box(id)
        })
    });

    // Benchmark: Invalidate
    group.bench_function("invalidate", |b| {
        let mut idx = 0usize;
        b.iter(|| {
            idx = (idx + 1) % tenant_ids.len();
            cache.invalidate(&tenant_ids[idx]);
            black_box(())
        })
    });

    group.finish();
}

fn bench_cache_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("tenant_cache_throughput");

    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let cache = TenantCache::new();
            let ids: Vec<TenantId> = (0..size).map(|_| TenantId::new_v4()).collect();

            // Pre-populate
            for id in &ids {
                let data = TenantData {
                    id: *id,
                    name: "test".to_string(),
                    config: HashMap::new(),
                };
                cache.insert(data);
            }

            b.iter(|| {
                for id in &ids {
                    let result = cache
                        .get(id)
                        .map(|tenant| (tenant.name.len(), tenant.config.len()));
                    black_box(result);
                }
            })
        });
    }

    group.finish();
}

fn bench_cache_contention(c: &mut Criterion) {
    use std::thread;

    let mut group = c.benchmark_group("tenant_cache_contention");

    group.bench_function("concurrent_reads", |b| {
        let cache = Arc::new(TenantCache::new());
        let id = TenantId::new_v4();
        cache.insert(TenantData {
            id,
            name: "test".to_string(),
            config: HashMap::new(),
        });

        b.iter(|| {
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let cache = cache.clone();
                    thread::spawn(move || {
                        for _ in 0..100 {
                            let result = cache
                                .get(&id)
                                .map(|tenant| (tenant.name.len(), tenant.config.len()));
                            black_box(result);
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
    tenant_cache_benches,
    bench_cache_operations,
    bench_cache_throughput,
    bench_cache_contention
);
criterion_main!(tenant_cache_benches);
