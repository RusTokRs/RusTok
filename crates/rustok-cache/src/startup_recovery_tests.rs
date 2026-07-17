use std::net::TcpListener;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::{Child, Command};

use crate::{CacheBackendOptions, CacheService};

fn reserve_loopback_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("loopback port should be reservable")
        .local_addr()
        .expect("reserved loopback address")
        .port()
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

    let url = format!("redis://127.0.0.1:{port}/");
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(client) = redis::Client::open(url.as_str())
                && let Ok(mut connection) = client.get_multiplexed_async_connection().await
                && redis::cmd("PING")
                    .query_async::<String>(&mut connection)
                    .await
                    .as_deref()
                    == Ok("PONG")
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("spawned Redis did not become ready");

    child
}

async fn stop_redis(child: &mut Child) {
    child.kill().await.expect("redis-server should stop");
    child.wait().await.expect("redis-server should be reaped");
}

#[tokio::test]
#[ignore = "requires redis-server via RUSTOK_CACHE_REDIS_SERVER_BIN"]
async fn raw_backend_created_during_startup_outage_connects_after_redis_recovers() {
    let binary = std::env::var("RUSTOK_CACHE_REDIS_SERVER_BIN")
        .expect("RUSTOK_CACHE_REDIS_SERVER_BIN must point to redis-server");
    let port = reserve_loopback_port();
    let url = format!("redis://127.0.0.1:{port}/");
    let prefix = format!("startup-recovery:{}", uuid::Uuid::new_v4().simple());
    let service = CacheService::from_url(Some(&url));
    let options = CacheBackendOptions::default();
    let backend = service
        .raw_shared_client_backend(&prefix, Duration::from_secs(30), 16, &options)
        .await;

    assert!(backend.health().await.is_err());
    backend
        .set("local".to_string(), b"bounded".to_vec())
        .await
        .expect("startup outage write should remain in bounded fallback");
    assert_eq!(
        backend.get("local").await.unwrap(),
        Some(b"bounded".to_vec())
    );

    let mut redis_process = spawn_redis(&binary, port).await;
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if backend.health().await.is_ok() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("existing backend did not connect after Redis startup");

    backend
        .set("shared".to_string(), b"redis".to_vec())
        .await
        .expect("recovered backend should write to Redis");

    let client = redis::Client::open(url.as_str()).unwrap();
    let mut connection = client.get_multiplexed_async_connection().await.unwrap();
    let stored = redis::cmd("GET")
        .arg(format!("{prefix}:shared"))
        .query_async::<Option<Vec<u8>>>(&mut connection)
        .await
        .unwrap();
    assert_eq!(stored, Some(b"redis".to_vec()));

    stop_redis(&mut redis_process).await;
}
