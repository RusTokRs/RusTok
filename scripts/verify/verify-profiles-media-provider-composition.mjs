#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function readRepo(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) {
    failures.push(`${relativePath}: expected file`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function requireMarker(source, marker, description) {
  if (!source.includes(marker)) failures.push(description);
}

function rejectMarker(source, marker, description) {
  if (source.includes(marker)) failures.push(description);
}

const runtimePath = "apps/server/src/services/profile_media_public_image_runtime.rs";
const deploymentPath =
  "apps/server/src/services/profile_media_public_image_deployment.rs";
const servicesPath = "apps/server/src/services/mod.rs";
const schemaPath = "apps/server/src/services/graphql_schema.rs";
const bootstrapPath = "apps/server/src/services/server_bootstrap.rs";
const routerPath = "apps/server/src/services/app_router.rs";
const serverCargoPath = "apps/server/Cargo.toml";
const transportCargoPath = "crates/rustok-media-transport/Cargo.toml";
const transportClientPath =
  "crates/rustok-media-transport/src/public_image_client.rs";
const transportLibPath = "crates/rustok-media-transport/src/lib.rs";
const profileGraphqlPath = "crates/rustok-profiles/src/graphql/types.rs";
const profileNativePath =
  "crates/rustok-profiles/storefront/src/transport/native_server_adapter.rs";

const runtime = readRepo(runtimePath);
const deployment = readRepo(deploymentPath);
const services = readRepo(servicesPath);
const schema = readRepo(schemaPath);
const bootstrap = readRepo(bootstrapPath);
const router = readRepo(routerPath);
const serverCargo = readRepo(serverCargoPath);
const transportCargo = readRepo(transportCargoPath);
const transportClient = readRepo(transportClientPath);
const transportLib = readRepo(transportLibPath);
const profileGraphql = readRepo(profileGraphqlPath);
const profileNative = readRepo(profileNativePath);

for (const marker of [
  "pub fn attach_profile_media_public_image_provider(",
  ".shared_get::<ProfileMediaPublicImageProvider>()",
  ".get::<ProfileMediaPublicImageProvider>()",
  ".shared_get::<rustok_storage::StorageRuntime>()",
  "MediaPublicImageService::new(ctx.db_clone(), storage)",
  "enriched.insert(selected.clone())",
  "ctx.shared_insert(selected)",
  "ctx.shared_insert(enriched.clone())",
  "deployment_seeded_provider_wins_and_reaches_host_runtime",
  "embedded_provider_is_registered_when_no_override_exists",
  "Arc::ptr_eq(&deployment_port, &host_provider.port())",
]) {
  requireMarker(runtime, marker, `${runtimePath}: missing composition marker ${marker}`);
}

const deploymentIndex = runtime.indexOf(
  ".shared_get::<ProfileMediaPublicImageProvider>()",
);
const extensionIndex = runtime.indexOf(
  ".get::<ProfileMediaPublicImageProvider>()",
);
const embeddedIndex = runtime.indexOf(
  ".shared_get::<rustok_storage::StorageRuntime>()",
);
if (
  deploymentIndex < 0 ||
  extensionIndex < 0 ||
  embeddedIndex < 0 ||
  !(deploymentIndex < extensionIndex && extensionIndex < embeddedIndex)
) {
  failures.push(
    `${runtimePath}: provider selection must prefer deployment override, then extension, then embedded storage`,
  );
}

for (const marker of [
  'const PROVIDER_ENV: &str = "RUSTOK_PROFILE_MEDIA_PROVIDER";',
  'const GRPC_ENDPOINT_ENV: &str = "RUSTOK_PROFILE_MEDIA_GRPC_ENDPOINT";',
  'const PUBLIC_ORIGIN_ENV: &str = "RUSTOK_PROFILE_MEDIA_PUBLIC_ORIGIN";',
  'const TLS_DOMAIN_ENV: &str = "RUSTOK_PROFILE_MEDIA_GRPC_TLS_DOMAIN";',
  'const CONNECT_TIMEOUT_MS_ENV: &str = "RUSTOK_PROFILE_MEDIA_GRPC_CONNECT_TIMEOUT_MS";',
  '"embedded" =>',
  '"grpc" =>',
  "remote Media variables require {PROVIDER_ENV}=grpc",
  "GrpcMediaPublicImageConnectionConfig::new(remote.endpoint)",
  ".with_public_origin(remote.public_origin.clone())",
  ".with_tls_domain(remote.tls_domain)",
  ".with_connect_timeout(Duration::from_millis(remote.connect_timeout_ms))",
  ".allow_insecure_loopback(remote.allow_insecure_loopback)",
  "connection.connect().await",
  "ctx.shared_insert(ProfileMediaPublicImageProvider::new(provider))",
  "remote_variables_are_not_silently_ignored_in_embedded_mode",
  "grpc_requires_an_endpoint",
]) {
  requireMarker(
    deployment,
    marker,
    `${deploymentPath}: missing deployment marker ${marker}`,
  );
}
for (const forbidden of [
  "endpoint = %",
  "endpoint = ?",
  "public_origin = %",
  "public_origin = ?",
]) {
  rejectMarker(
    deployment,
    forbidden,
    `${deploymentPath}: deployment logs must not expose endpoint/origin values`,
  );
}

for (const marker of [
  "pub struct GrpcMediaPublicImageConnectionConfig",
  "pub struct GrpcMediaPublicImageProvider",
  "impl MediaPublicImageReadPort for GrpcMediaPublicImageProvider",
  "Endpoint::from_shared(validated.endpoint)",
  ".connect_timeout(validated.connect_timeout)",
  "ClientTlsConfig::new().with_webpki_roots()",
  '"http" if allow_insecure_loopback && is_loopback_host(parsed.host())',
  "InsecureEndpointForbidden",
  "InsecurePublicOriginForbidden",
  "MAX_CONNECT_TIMEOUT_MS",
  "rebase_public_descriptor(asset.descriptor.as_mut(), self.public_origin.as_deref())",
  "descriptor.url.starts_with('/') && !descriptor.url.starts_with(\"//\")",
  "public_origin_rebases_only_root_relative_descriptors",
  "external_plaintext_endpoint_is_rejected",
  "plaintext_loopback_requires_explicit_opt_in",
]) {
  requireMarker(
    transportClient,
    marker,
    `${transportClientPath}: missing secure remote marker ${marker}`,
  );
}
for (const forbidden of [
  "MediaPublicImageBody",
  "read_public_image(",
  "object_store",
]) {
  rejectMarker(
    transportClient,
    forbidden,
    `${transportClientPath}: remote descriptor adapter must not own image bytes/storage`,
  );
}

for (const marker of [
  "pub mod public_image_client;",
  "GrpcMediaPublicImageConnectionConfig",
  "GrpcMediaPublicImageProvider",
]) {
  requireMarker(transportLib, marker, `${transportLibPath}: missing export ${marker}`);
}
for (const marker of [
  'tonic = { workspace = true, features = ["tls-ring", "tls-webpki-roots"] }',
  "thiserror.workspace = true",
  "url.workspace = true",
]) {
  requireMarker(
    transportCargo,
    marker,
    `${transportCargoPath}: missing dependency marker ${marker}`,
  );
}
for (const marker of [
  'mod-profiles  = ["dep:rustok-profiles", "dep:rustok-media-transport"',
  "rustok-media-transport = { workspace = true, optional = true }",
]) {
  requireMarker(serverCargo, marker, `${serverCargoPath}: missing remote Media wiring ${marker}`);
}

for (const marker of [
  "pub mod profile_media_public_image_deployment;",
  "pub mod profile_media_public_image_runtime;",
]) {
  requireMarker(services, marker, `${servicesPath}: provider module is not wired: ${marker}`);
}
for (const marker of [
  "configure_profile_media_public_image_deployment(&runtime_ctx).await?;",
  "bootstrap_app_runtime(runtime_ctx.clone(), auth_config.clone(), &rustok_settings).await?;",
]) {
  requireMarker(bootstrap, marker, `${bootstrapPath}: missing startup marker ${marker}`);
}
const configureIndex = bootstrap.indexOf(
  "configure_profile_media_public_image_deployment(&runtime_ctx).await?;",
);
const bootstrapIndex = bootstrap.indexOf(
  "bootstrap_app_runtime(runtime_ctx.clone(), auth_config.clone(), &rustok_settings).await?;",
);
if (
  configureIndex < 0 ||
  bootstrapIndex < 0 ||
  configureIndex > bootstrapIndex
) {
  failures.push(
    `${bootstrapPath}: remote provider must be configured before runtime/schema materialization`,
  );
}

for (const marker of [
  "attach_profile_media_public_image_provider",
  "let runtime_extensions = attach_profile_media_public_image_provider(",
  "module_runtime_extensions_from_ctx(ctx)",
  "runtime_extensions.apply_to_host_runtime(host_runtime)",
  "runtime_extensions,",
]) {
  requireMarker(schema, marker, `${schemaPath}: missing shared snapshot marker ${marker}`);
}
const attachIndex = schema.indexOf(
  "let runtime_extensions = attach_profile_media_public_image_provider(",
);
const graphqlInputsIndex = schema.indexOf(
  "let graphql_runtime_inputs = rustok_api::graphql::GraphqlRuntimeInputs::new(host_runtime);",
);
if (attachIndex < 0 || graphqlInputsIndex < 0 || attachIndex > graphqlInputsIndex) {
  failures.push(
    `${schemaPath}: provider must be attached before GraphQL runtime inputs are materialized`,
  );
}

for (const marker of [
  "shared_get::<Arc<ModuleRuntimeExtensions>>()",
  ".apply_to_host_runtime(runtime_ctx)",
  ".with_shared_value(extensions)",
]) {
  requireMarker(router, marker, `${routerPath}: missing server-function extension transfer ${marker}`);
}

for (const [source, sourcePath] of [
  [profileGraphql, profileGraphqlPath],
  [profileNative, profileNativePath],
]) {
  requireMarker(
    source,
    "ProfileMediaPublicImageProvider",
    `${sourcePath}: typed provider wrapper must be consumed`,
  );
  requireMarker(
    source,
    "MediaPublicImageReadPort",
    `${sourcePath}: owner public-image port must remain the consumer boundary`,
  );
  for (const forbidden of [
    "GrpcMediaProvider",
    "GrpcMediaPublicImageProvider",
    "RUSTOK_PROFILE_MEDIA_",
    "tonic::",
  ]) {
    rejectMarker(
      source,
      forbidden,
      `${sourcePath}: Profiles consumer must not know deployment transport marker ${forbidden}`,
    );
  }
}

if (failures.length > 0) {
  console.error("Profiles Media provider composition verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Profiles Media provider composition verification passed");
