use bytes::Bytes;
use futures_util::TryStreamExt;
use object_store::{ObjectStoreExt, PutMode, PutOptions, path::Path};
use rustok_storage::{LocalStorageConfig, StorageRuntime};
use uuid::Uuid;

async fn exercise_runtime(runtime: &StorageRuntime) {
    let root = format!("conformance/{}/", Uuid::new_v4());
    let object = Path::from(format!("{root}object.bin"));
    let multipart = Path::from(format!("{root}aborted.bin"));
    let payload = Bytes::from_static(b"rustok-object-store-conformance");

    runtime
        .objects
        .put_opts(
            &object,
            payload.clone().into(),
            PutOptions {
                mode: PutMode::Create,
                ..runtime.put_options("application/octet-stream")
            },
        )
        .await
        .expect("conditional create should store a new object");
    let duplicate = runtime
        .objects
        .put_opts(
            &object,
            payload.clone().into(),
            PutOptions {
                mode: PutMode::Create,
                ..runtime.put_options("application/octet-stream")
            },
        )
        .await
        .expect_err("conditional create should reject an existing object");
    assert!(matches!(
        duplicate,
        object_store::Error::AlreadyExists { .. } | object_store::Error::Precondition { .. }
    ));

    let bytes = runtime
        .objects
        .get(&object)
        .await
        .expect("object should be readable")
        .bytes()
        .await
        .expect("object body should be readable");
    assert_eq!(bytes, payload);
    let listed = runtime
        .objects
        .list(Some(&Path::from(root.as_str())))
        .try_collect::<Vec<_>>()
        .await
        .expect("prefix listing should succeed");
    assert!(listed.iter().any(|meta| meta.location == object));

    let mut upload = runtime
        .objects
        .put_multipart(&multipart)
        .await
        .expect("multipart upload should start");
    upload
        .put_part(Bytes::from(vec![7_u8; 5 * 1024 * 1024]).into())
        .await
        .expect("multipart part should upload");
    upload.abort().await.expect("multipart upload should abort");
    assert!(matches!(
        runtime.objects.head(&multipart).await,
        Err(object_store::Error::NotFound { .. })
    ));

    runtime
        .objects
        .delete(&object)
        .await
        .expect("test object should delete");
}

#[tokio::test]
async fn local_backend_conforms_to_required_object_operations() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let runtime = StorageRuntime::local(&LocalStorageConfig {
        base_dir: directory.path().display().to_string(),
        base_url: "/media".to_string(),
        fsync: false,
    })
    .expect("local runtime should initialize");
    exercise_runtime(&runtime).await;
    assert!(
        runtime
            .signed_upload_url(&Path::from("unsigned"), std::time::Duration::from_secs(60))
            .await
            .expect("local signer lookup should succeed")
            .is_none()
    );
}

#[cfg(feature = "s3")]
#[tokio::test]
async fn s3_compatible_backend_conforms_when_environment_is_configured() {
    let Some(endpoint) = std::env::var("RUSTOK_TEST_S3_ENDPOINT").ok() else {
        eprintln!("skipping S3 conformance: RUSTOK_TEST_S3_ENDPOINT is not set");
        return;
    };
    let runtime = StorageRuntime::from_config(&rustok_storage::StorageConfig {
        driver: rustok_storage::StorageDriver::S3,
        local: LocalStorageConfig::default(),
        s3: rustok_storage::S3StorageConfig {
            bucket: std::env::var("RUSTOK_TEST_S3_BUCKET")
                .unwrap_or_else(|_| "rustok-media-test".to_string()),
            region: Some(
                std::env::var("RUSTOK_TEST_S3_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
            ),
            endpoint_url: Some(endpoint),
            access_key_id: Some(
                std::env::var("RUSTOK_TEST_S3_ACCESS_KEY")
                    .unwrap_or_else(|_| "rustok-test".to_string()),
            ),
            secret_access_key: Some(
                std::env::var("RUSTOK_TEST_S3_SECRET_KEY")
                    .unwrap_or_else(|_| "rustok-test-secret".to_string()),
            ),
            session_token: None,
            public_base_url: None,
            allow_http: true,
            virtual_hosted_style_request: false,
        },
    })
    .await
    .expect("S3-compatible runtime should initialize");

    exercise_runtime(&runtime).await;
    let path = Path::from("conformance/signed-put.bin");
    let upload_url = runtime
        .signed_upload_url(&path, std::time::Duration::from_secs(60))
        .await
        .expect("signed PUT should succeed")
        .expect("S3 runtime should provide a signer");
    let download_url = runtime
        .signed_download_url(&path, std::time::Duration::from_secs(60))
        .await
        .expect("signed GET should succeed")
        .expect("S3 runtime should provide a signer");
    assert!(upload_url.contains("X-Amz-Signature="));
    assert!(download_url.contains("X-Amz-Signature="));
}
