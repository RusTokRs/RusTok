import fs from 'node:fs';

function read(path) { return fs.readFileSync(path, 'utf8'); }
function json(path) { return JSON.parse(read(path)); }
function fail(message) {
  console.error('[verify-forum-attachment-hold-reconciliation] ' + message);
  process.exit(1);
}
function hasAll(source, markers, label) {
  for (const marker of markers) {
    if (!source.includes(marker)) fail(label + ' missing ' + marker);
  }
}

const contractPath = 'crates/modules/rustok-forum/contracts/forum-attachment-hold-reconciliation.json';
const contract = json(contractPath);
if (contract.schema_version !== 1) fail('schema_version drift');
if (contract.task !== 'FORUM-33-ATTACHMENT-HOLDS') fail('task identity drift');
if (contract.operation?.name !== 'forum_attachment_hold_reconciliation_report') fail('operation name drift');
if (contract.operation?.transport !== 'GraphQL') fail('transport drift');
if (contract.operation?.pagination?.default_limit !== 100 || contract.operation?.pagination?.max_limit !== 500) {
  fail('pagination bounds drift');
}
if (contract.operation?.repair !== false) fail('repair must remain disabled');
if (contract.runtime_composition?.host_composes_embedded_media_reader !== true) {
  fail('embedded host composition contract drift');
}
if (contract.runtime_composition?.forum_consumes_typed_port_only !== true) {
  fail('Forum typed-port consumption contract drift');
}
if (contract.runtime_composition?.missing_reader_behavior !== 'FORUM_MEDIA_REFERENCE_LIST_CAPABILITY_UNAVAILABLE') {
  fail('missing Media reader capability code drift');
}

const source = read('crates/modules/rustok-forum/src/services/attachment_hold_reconciliation.rs');
hasAll(source, [
  'pub struct ForumAttachmentHoldReconciliationService',
  'MediaAssetReferenceListRequest',
  'list_asset_references(',
  'owner_module: FORUM_MEDIA_OWNER_MODULE.to_string()',
  'OrphanMediaHold',
  'MediaReferenceMismatch',
  'validate_media_reference(',
  'enforce_scope(security, Resource::ForumCategories, Action::Manage)',
  'enforce_scope(security, Resource::ForumTopics, Action::Manage)',
], 'reconciliation source');

for (const forbidden of [
  'media_asset_reference_holds',
  'media_assets',
  'media_blobs',
  'retain_asset_reference',
  'release_asset_reference',
]) {
  if (source.includes(forbidden)) fail('Forum reconciliation crosses forbidden Media boundary: ' + forbidden);
}

const graphql = read('crates/modules/rustok-forum/src/graphql/reconciliation_query.rs');
hasAll(graphql, [
  'forum_attachment_hold_reconciliation_report',
  'ForumAttachmentHoldReconciliationService::new(db, media)',
  'attachment_hold_reconciliation_media()',
  'media_after: Option<Uuid>',
], 'GraphQL source');

const runtime = read('crates/modules/rustok-forum/src/graphql/runtime_data.rs');
hasAll(runtime, [
  'shared_get::<Arc<dyn MediaAssetReadPort>>()',
  'attachment_hold_media: Option<Arc<dyn MediaAssetReadPort>>',
], 'Forum GraphQL runtime data');

const host = read('apps/server/src/services/graphql_schema.rs');
hasAll(host, [
  'attach_forum_media_asset_read_provider',
  'Arc<dyn MediaAssetReadPort>',
  'MediaService::new(ctx.db_clone(), storage)',
], 'server host composition');

const mediaPorts = read('crates/modules/rustok-media/src/ports.rs');
hasAll(mediaPorts, [
  'pub struct MediaAssetReferenceListRequest',
  'pub struct MediaAssetReferenceListPage',
  'async fn list_asset_references(',
], 'Media read contract');

const transportProto = read('crates/modules/rustok-media-transport/proto/rustok/media/media.proto');
hasAll(transportProto, ['rpc ListAssetReferences(JsonRequest) returns (JsonResponse);'], 'Media gRPC contract');

const forumManifest = read('crates/modules/rustok-forum/rustok-module.toml');
if (!forumManifest.includes('media = { version_req = ">=0.1.0" }')) fail('Forum manifest must depend on Media');

const graphqlTest = read('crates/modules/rustok-forum/tests/reconciliation_graphql_contract.rs');
hasAll(graphqlTest, [
  'GqlForumAttachmentHoldReconciliationReport',
  'forumAttachmentHoldReconciliationReport',
  'attachment-hold',
], 'GraphQL contract test');

console.log('[verify-forum-attachment-hold-reconciliation] source boundaries and contract metadata are consistent');
