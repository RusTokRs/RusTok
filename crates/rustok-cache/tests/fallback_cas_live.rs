use std::net::TcpListener;
use std::process::Stdio;
use std::time::Duration;

use rustok_cache::{CacheBackendOptions, CacheCompareAndSetOutcome, CacheService};
use rustok_core::CircuitBreakerConfig;
use tokio::process::{Child, Command};

fn reserve_loopback_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("loopback port should be reservable")
        .local_addr()
        .expect("reserved loopback address")
        .port()
}

async fn wait_for_redis(url: &str) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(client) = redis::Client::open(url)
                && let Ok(mut connection) = client.get_multiplexed_async_connection().await
            {
                let pong = redis::cmd("PING")
                    .query_async::<String>(&mut connection)
                    .await;
                if pong.as_deref() == Ok("PONG") {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("spawned Redis did not become ready");
}

async fn spawn_redis(binary: &str, port: u16) -> Child {
    let child = Command::new(binary)
        .arg("--bind")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(port.to_string())
        .arg("--save")
        .arg("")
        .arg("--appendonly")
        .arg("no")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("redis-server should start");
    wait_for_redis(&format!("redis://127.0.0.1:{port}/")).await;
    child
}

async fn stop_redis(child: &mut Child) {
    child.kill().await.expect("redis-server should stop");
    child.wait().await.expect("redis-server should be reaped");
}

fn fast_recovery_options() -> CacheBackendOptions {
    let mut options = CacheBackendOptions::default();
    options.redis_circuit_breaker = CircuitBreakerConfig {
        failure_threshold: 1,
        success_threshold: 1,
        timeout: Duration::from_millis(200),
        half_open_max_requests: Some(1),
    };
    options
}

async fn wait_for_backend_health(backend: &dyn rustok_core::CacheBackend) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if backend.health().await.is_ok() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("fallback Redis backend did not recover");
}

#[tokio::test]
#[ignore = "requires redis-server via RUSTOK_CACHE_REDIS_SERVER_BIN"]
async fn fallback_cas_fails_closed_during_redis_outage_and_recovers() {
    let binary = std::env::var("RUSTOK_CACHE_REDIS_SERVER_BIN")
        .expect("RUSTOK_CACHE_REDIS_SERVER_BIN must point to redis-server");
    let port = reserve_loopback_port();
    let url = format!("redis://127.0.0.1:{port}/");
    let mut redis_process = spawn_redis(binary.as_str(), port).await;

    let service = CacheService::from_url_with_options(Some(&url), fast_recovery_options());
    let backend = service
        .backend(
            &format!("fallback-cas-live-{}", uuid::Uuid::new_v4()),
            Duration::from_secs(30),
            128,
        )
        .await;
    backend
        .health()
        .await
        .expect("fallback Redis backend should start healthy");

    let original = vec![0x00, 0xff, 0x80, b'R', 0x00];
    let replacement = vec![0xfe, 0x00, 0x81, b'S', 0xff];
    backend
        .set("primary-cas".to_string(), original.clone())
        .await
        .expect("initial shared value should be written and mirrored");

    stop_redis(&mut redis_process).await;

    backend
        .compare_and_set("primary-cas", &original, replacement.clone(), None)
        .await
        .expect_err("CAS must fail while the shared primary is unavailable");
    assert_eq!(
        backend
            .get("primary-cas")
            .await
            .expect("bounded fallback should remain readable"),
        Some(original.clone()),
        "failed shared CAS must not mutate the local mirror"
    );

    let degraded_value = vec![0x10, 0x00, 0xff, 0x11];
    backend
        .set("degraded-write".to_string(), degraded_value.clone())
        .await
        .expect("bounded degraded write should remain locally available");
    let unsynchronized = backend
        .compare_and_set(
            "degraded-write",
            &degraded_value,
            b"must-not-apply".to_vec(),
            None,
        )
        .await
        .expect_err("CAS must reject unsynchronized local and shared state");
    assert!(
        unsynchronized
            .to_string()
            .contains("local and shared state are unsynchronized")
    );
    assert_eq!(
        backend
            .get("degraded-write")
            .await
            .expect("degraded local value should remain readable"),
        Some(degraded_value)
    );

    redis_process = spawn_redis(binary.as_str(), port).await;
    wait_for_backend_health(backend.as_ref()).await;

    backend
        .set("primary-cas".to_string(), original.clone())
        .await
        .expect("recovered primary should accept a fresh baseline");
    assert_eq!(
        backend
            .compare_and_set("primary-cas", &original, replacement.clone(), None)
            .await
            .expect("CAS should recover with the shared primary"),
        CacheCompareAndSetOutcome::Applied
    );
    assert_eq!(
        backend
            .get("primary-cas")
            .await
            .expect("recovered CAS value should be readable"),
        Some(replacement)
    );

    stop_redis(&mut redis_process).await;
}
