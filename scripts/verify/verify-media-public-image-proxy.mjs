#!/usr/bin/env node
// Media-owned public-image proxy, remote descriptor parity, runtime provider selection,
// and Profiles consumer guardrails.

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function readRepo(relativePath) {
  return readFileSync(repoPath(relativePath), "utf8");
}

function fail(message) {
  failures.push(message);
}

function assertExists(relativePath) {
  if (!existsSync(repoPath(relativePath))) fail(`${relativePath}: expected file`);
}

function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) fail(description);
}

function assertNotContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (found) fail(description);
}

const paths = {
  mediaPublic: "crates/rustok-media/src/public_image.rs",
  mediaLib: "crates/rustok-media/src/lib.rs",
  mediaController: "crates/rustok-media/src/controllers/mod.rs",
  mediaTest: "crates/rustok-media/tests/public_image_proxy.rs",
  transportProto: "crates/rustok-media-transport/proto/rustok/media/media.proto",
  transportClient: "crates/rustok-media-transport/src/client.rs",
  transportServer: "crates/rustok-media-transport/src/server.rs",
  transportTest: "crates/rustok-media-transport/tests/port_conformance.rs",
  profileMedia: "crates/rustok-profiles/src/media.rs",
  profileGraphql: "crates/rustok-profiles/src/graphql/types.rs",
  profileNative: "crates/rustok-profiles/storefront/src/transport/native_server_adapter.rs",
};

for (const value of Object.values(paths)) assertExists(value);

const mediaPublic = readRepo(paths.mediaPublic);
const mediaLib = readRepo(paths.mediaLib);
const mediaController = readRepo(paths.mediaController);
const mediaTest = readRepo(paths.mediaTest);
const transportProto = readRepo(paths.transportProto);
const transportClient = readRepo(paths.transportClient);
const transportServer = readRepo(paths.transportServer);
const transportTest = readRepo(paths.transportTest);
const profileMedia = readRepo(paths.profileMedia);
const profileGraphql = readRepo(paths.profileGraphql);
const profileNative = readRepo(paths.profileNative);

for (const marker of [
  "pub trait MediaPublicImageReadPort",
  "pub struct MediaPublicImageService",
  "MediaImagePublicUrlPolicy::DirectPublic",
  "MediaImagePublicUrlPolicy::ProxyRequired",
  "MediaImagePublicUrlPolicy::NotAddressable",
  "blob.checksum_sha256",
  "BlobState::Ready",
  'blob.mime_type.starts_with("image/")',
  "context.require_policy(PortCallPolicy::read())",
]) {
  assertContains(mediaPublic, marker, `${paths.mediaPublic}: missing owner marker ${marker}`);
}
assertContains(mediaLib, "pub mod public_image;", `${paths.mediaLib}: public image module not wired`);
assertContains(mediaLib, "MediaPublicImageReadPort", `${paths.mediaLib}: public image port not exported`);

for (const marker of [
  '"/api/media/public/images/{id}/{checksum_sha256}"',
  "read_public_image(tenant.id, id, &checksum_sha256)",
  "PUBLIC_IMAGE_CACHE_CONTROL",
  "max-age=31536000, immutable",
  "IF_NONE_MATCH",
  "ETAG",
  '"x-content-type-options", "nosniff"',
]) {
  assertContains(mediaController, marker, `${paths.mediaController}: missing HTTP marker ${marker}`);
}
assertNotContains(
  mediaController.slice(
    mediaController.indexOf("pub async fn public_image("),
    mediaController.indexOf("/// Delete a media asset."),
  ),
  "AuthContext",
  `${paths.mediaController}: capability route must not require authenticated media-library access`,
);

assertContains(
  transportProto,
  "rpc GetPublicImageAsset(ImageDescriptorRequest) returns (JsonResponse);",
  `${paths.transportProto}: public image owner RPC missing`,
);
assertNotContains(
  transportProto,
  /GetPublicImage(?:Body|Bytes|Download)/,
  `${paths.transportProto}: binary image delivery must not enter gRPC`,
);
for (const marker of [
  "impl MediaPublicImageReadPort for GrpcMediaProvider",
  ".get_public_image_asset(with_deadline(payload, &context))",
  "Result<MediaPublicImageAsset, PortError>",
]) {
  assertContains(transportClient, marker, `${paths.transportClient}: missing remote client marker ${marker}`);
}
for (const marker of [
  "public_image_provider: Option<Arc<dyn MediaPublicImageReadPort>>",
  "MediaGrpcOperation::GetPublicImageAsset",
  "pub fn with_public_image_provider(",
  '"media.public_image_provider_unavailable"',
  ".get_public_image_asset(context, parse_id(&request.id)?, request.alt)",
]) {
  assertContains(transportServer, marker, `${paths.transportServer}: missing remote server marker ${marker}`);
}
assertNotContains(
  transportServer,
  "read_public_image(",
  `${paths.transportServer}: gRPC server must not read or return image bodies`,
);
for (const marker of [
  "MediaGrpcOperation::GetPublicImageAsset",
  ".with_public_image_provider(public_images.clone())",
  "storage-relative image should receive an owner capability URL",
  "public image deadline policy should cross the provider boundary",
  "deleted public image should retain typed not-found semantics",
  "exercise_provider(&remote, &remote, &remote",
]) {
  assertContains(transportTest, marker, `${paths.transportTest}: missing parity scenario ${marker}`);
}

for (const marker of [
  "pub struct ProfileMediaPublicImageProvider",
  "Arc<dyn MediaPublicImageReadPort>",
  "pub fn port(&self) -> Arc<dyn MediaPublicImageReadPort>",
]) {
  assertContains(profileMedia, marker, `${paths.profileMedia}: missing runtime provider marker ${marker}`);
}

for (const [source, sourcePath] of [
  [profileGraphql, paths.profileGraphql],
  [profileNative, paths.profileNative],
]) {
  assertContains(source, "MediaPublicImageReadPort", `${sourcePath}: must use Media public image owner port`);
  assertContains(source, "ProfileMediaPublicImageProvider", `${sourcePath}: must select a typed runtime provider`);
  assertContains(source, "get_public_image_asset", `${sourcePath}: must request owner descriptor`);
  assertContains(source, "validate_profile_media_asset", `${sourcePath}: must revalidate profile uploader/tenant/MIME`);
  assertNotContains(source, "GrpcMediaProvider", `${sourcePath}: consumer must not know the gRPC adapter type`);
  assertNotContains(source, "tonic::", `${sourcePath}: consumer must not own gRPC framing`);
  assertNotContains(source, "public_image_path(", `${sourcePath}: consumer must not construct Media capability URLs`);
  assertNotContains(source, '"/api/media/public/images', `${sourcePath}: consumer must not own Media route strings`);
}
for (const marker of [
  "data_opt::<Arc<ModuleRuntimeExtensions>>()",
  "extensions.get::<ProfileMediaPublicImageProvider>()",
  "MediaPublicImageService::new(",
]) {
  assertContains(profileGraphql, marker, `${paths.profileGraphql}: missing provider-selection marker ${marker}`);
}
for (const marker of [
  "shared_get::<ProfileMediaPublicImageProvider>()",
  "Arc<dyn MediaPublicImageReadPort>",
  "media: &dyn rustok_media::MediaPublicImageReadPort",
  "MediaPublicImageService::new(",
]) {
  assertContains(profileNative, marker, `${paths.profileNative}: missing provider-selection marker ${marker}`);
}

for (const marker of [
  "assert_eq!(item.public_url, item.storage_path)",
  "starts_with(&expected_prefix)",
  "wrong checksum must not expose the object",
  "cross-tenant capability must not expose the object",
  'direct.url.starts_with("/media/")',
]) {
  assertContains(mediaTest, marker, `${paths.mediaTest}: missing scenario ${marker}`);
}

if (failures.length > 0) {
  console.error("Media public image proxy verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Media public image proxy verification passed");
