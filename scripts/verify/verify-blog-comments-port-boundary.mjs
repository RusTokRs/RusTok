#!/usr/bin/env node

import './verify-blog-comments-http-port-injection.mjs';
import './verify-blog-comments-graphql-port-injection.mjs';
import './verify-blog-comments-storefront-native-port-injection.mjs';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve('.');
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function read(relativePath) {
  const target = repoPath(relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return '';
  }
  return readFileSync(target, 'utf8');
}

function json(relativePath) {
  try {
    return JSON.parse(read(relativePath));
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return null;
  }
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function requireNoMarker(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

function sameSet(actual, expected) {
  return [...actual].sort().join('|') === [...expected].sort().join('|');
}

const evidencePath = 'crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json';
const fallbackEvidencePath = 'crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json';
const servicePath = 'crates/rustok-blog/src/services/comment.rs';
const graphqlOwnerPath = 'crates/rustok-blog/src/graphql/types.rs';
const storefrontModelPath = 'crates/rustok-blog/storefront/src/model.rs';
const storefrontGraphqlPath = 'crates/rustok-blog/storefront/src/transport/graphql_adapter.rs';
const storefrontNativePath = 'crates/rustok-blog/storefront/src/transport/native_server_adapter.rs';
const storefrontUiPath = 'crates/rustok-blog/storefront/src/ui/leptos.rs';
const providerRegistryPath = 'crates/rustok-comments/contracts/comments-fba-registry.json';
const consumerRegistryPath = 'crates/rustok-blog/contracts/blog-fba-registry.json';
const planPath = 'crates/rustok-blog/docs/implementation-plan.md';
const injectionConstructor = 'CommentService::with_comments_thread_port';
const injectionSignature = 'fn(DatabaseConnection, Arc<dyn CommentsThreadPort>) -> CommentService';
const injectionTest =
  'services::comment::port_injection_tests::comment_service_accepts_an_injected_comments_thread_port';
const injectionCommand =
  `cargo test -p rustok-blog --lib ${injectionTest} -- --exact`;
const expectedOperations = [
  'create_comment',
  'get_comment',
  'list_comments_for_target',
  'list_public_comments_for_target',
  'update_comment',
  'set_comment_status',
  'delete_comment',
];

const evidence = json(evidencePath);
const fallbackEvidence = json(fallbackEvidencePath);
const providerRegistry = json(providerRegistryPath);
const consumerRegistry = json(consumerRegistryPath);
const service = read(servicePath);
const graphqlOwner = read(graphqlOwnerPath);
const storefrontModel = read(storefrontModelPath);
const storefrontGraphql = read(storefrontGraphqlPath);
const storefrontNative = read(storefrontNativePath);
const storefrontUi = read(storefrontUiPath);
const plan = read(planPath);

if (evidence) {
  if (evidence.schema_version !== 3) failures.push(`${evidencePath}: schema_version drift`);
  if (
    evidence.module !== 'blog' ||
    evidence.surface !== 'comments_port_boundary' ||
    evidence.role !== 'consumer' ||
    evidence.provider !== 'comments'
  ) failures.push(`${evidencePath}: identity drift`);
  if (evidence.status !== 'source_verified_no_compile') failures.push(`${evidencePath}: status drift`);
  if (evidence.compile_policy !== 'not_run_by_request') failures.push(`${evidencePath}: compile policy drift`);
  if (
    evidence.source_contract?.consumer_service !== servicePath ||
    evidence.source_contract?.provider_registry !== providerRegistryPath ||
    evidence.source_contract?.consumer_registry !== consumerRegistryPath ||
    evidence.source_contract?.injection_constructor !== injectionConstructor
  ) failures.push(`${evidencePath}: source contract path drift`);
  if (!sameSet(evidence.profiles?.source_verified ?? [], ['in_process'])) {
    failures.push(`${evidencePath}: source-verified profile drift`);
  }
  if (!sameSet(evidence.profiles?.pending ?? [], ['remote_adapter_placeholder'])) {
    failures.push(`${evidencePath}: pending profile drift`);
  }
  const injection = evidence.adapter_injection ?? {};
  if (
    injection.status !== 'executable_no_run' ||
    injection.runtime_status !== 'not_run' ||
    injection.source !== servicePath ||
    injection.constructor !== injectionConstructor ||
    injection.signature !== injectionSignature ||
    injection.test !== injectionTest ||
    injection.command !== injectionCommand ||
    injection.default_profile !== 'in_process' ||
    injection.remote_transport_implementation !== 'pending'
  ) failures.push(`${evidencePath}: adapter injection drift`);
  if (!sameSet((evidence.cases ?? []).map((entry) => entry.operation), expectedOperations)) {
    failures.push(`${evidencePath}: operation set drift`);
  }
  for (const entry of evidence.cases ?? []) {
    if (entry.runtime_evidence !== 'pending') {
      failures.push(`${evidencePath}: ${entry.operation} runtime status drift`);
    }
  }
  if (
    evidence.fallback_smoke?.status !== 'planned' ||
    evidence.fallback_smoke?.runtime_evidence !== 'pending'
  ) failures.push(`${evidencePath}: fallback status drift`);
}

if (fallbackEvidence) {
  if (fallbackEvidence.schema_version !== 2) failures.push(`${fallbackEvidencePath}: schema_version drift`);
  if (
    fallbackEvidence.module !== 'blog' ||
    fallbackEvidence.role !== 'consumer' ||
    fallbackEvidence.provider !== 'comments'
  ) failures.push(`${fallbackEvidencePath}: identity drift`);
  if (fallbackEvidence.status !== 'source_verified_no_compile') failures.push(`${fallbackEvidencePath}: status drift`);
  if (fallbackEvidence.compile_policy !== 'not_run_by_request' || fallbackEvidence.runtime_status !== 'not_run') {
    failures.push(`${fallbackEvidencePath}: execution policy drift`);
  }
  if (fallbackEvidence.runner !== 'scripts/verify/verify-blog-comments-port-boundary.mjs') {
    failures.push(`${fallbackEvidencePath}: runner drift`);
  }
  const expectedSources = {
    consumer_service: servicePath,
    consumer_error_mapping: servicePath,
    provider_port_registry: providerRegistryPath,
    graphql_owner: graphqlOwnerPath,
    storefront_model: storefrontModelPath,
    storefront_graphql: storefrontGraphqlPath,
    storefront_native: storefrontNativePath,
    storefront_ui: storefrontUiPath,
  };
  for (const [key, expected] of Object.entries(expectedSources)) {
    if (fallbackEvidence.source_contract?.[key] !== expected) {
      failures.push(`${fallbackEvidencePath}: ${key} source path drift`);
    }
  }
  const readDegradation = fallbackEvidence.storefront_read_degradation ?? {};
  if (
    readDegradation.status !== 'source_verified_no_compile' ||
    readDegradation.runtime_status !== 'not_run' ||
    readDegradation.operation !== 'list_public_comments_for_target' ||
    readDegradation.propagated_error_policy !== 'all_other_blog_errors' ||
    readDegradation.cached_thread_snapshot !== 'planned' ||
    readDegradation.comment_form_fallback !== 'planned' ||
    readDegradation.runtime_evidence !== 'pending'
  ) failures.push(`${fallbackEvidencePath}: storefront read degradation status drift`);
  if (!sameSet(readDegradation.transports ?? [], ['graphql', 'native_ssr'])) {
    failures.push(`${fallbackEvidencePath}: storefront transport parity drift`);
  }
  if (!sameSet(readDegradation.availability_states ?? [], ['AVAILABLE', 'UNAVAILABLE', 'TIMEOUT'])) {
    failures.push(`${fallbackEvidencePath}: availability state drift`);
  }
  if (!sameSet(readDegradation.degraded_error_kinds ?? [], ['ExternalService', 'Timeout'])) {
    failures.push(`${fallbackEvidencePath}: degraded error-kind drift`);
  }
  if (
    !Array.isArray(readDegradation.degraded_payload?.items) ||
    readDegradation.degraded_payload.items.length !== 0 ||
    readDegradation.degraded_payload?.total !== 0
  ) failures.push(`${fallbackEvidencePath}: degraded payload drift`);
  if (
    fallbackEvidence.fallback_smoke?.status !== 'planned' ||
    fallbackEvidence.fallback_smoke?.runtime_evidence !== 'pending'
  ) failures.push(`${fallbackEvidencePath}: degraded-mode status drift`);
}

const providerOperations = providerRegistry?.ports?.find((entry) => entry.name === 'CommentsThreadPort')?.operations ?? [];
if (!sameSet(providerOperations, expectedOperations)) failures.push(`${providerRegistryPath}: port operation drift`);
const dependency = consumerRegistry?.provider_dependencies?.find((entry) => entry.module === 'comments');
if (!dependency || dependency.port !== 'CommentsThreadPort') failures.push(`${consumerRegistryPath}: Comments dependency drift`);
if (!sameSet(dependency?.operations ?? [], expectedOperations)) failures.push(`${consumerRegistryPath}: dependency operation drift`);
if (!sameSet((consumerRegistry?.contract_tests?.cases ?? []).map((entry) => entry.operation), expectedOperations)) {
  failures.push(`${consumerRegistryPath}: contract-test operation drift`);
}
const registryInjection = consumerRegistry?.contract_tests?.adapter_injection ?? {};
if (
  registryInjection.status !== 'executable_no_run' ||
  registryInjection.runtime_status !== 'not_run' ||
  registryInjection.source !== servicePath ||
  registryInjection.constructor !== injectionConstructor ||
  registryInjection.signature !== injectionSignature ||
  registryInjection.test !== injectionTest ||
  registryInjection.command !== injectionCommand ||
  registryInjection.remote_transport_implementation !== 'pending'
) failures.push(`${consumerRegistryPath}: adapter injection drift`);
if (
  consumerRegistry?.contract_tests?.status !== 'source_verified_no_compile' ||
  consumerRegistry?.contract_tests?.runtime_status !== 'pending' ||
  consumerRegistry?.contract_tests?.fallback_smoke?.status !== 'planned'
) failures.push(`${consumerRegistryPath}: contract-test status drift`);

for (const marker of [
  'comments_thread_port: Arc<dyn CommentsThreadPort>',
  'let comments_thread_port = in_process_comments_thread_port(db.clone(), event_bus);',
  'Self::with_comments_thread_port(db, comments_thread_port)',
  'pub fn with_comments_thread_port(',
  'comments_thread_port: Arc<dyn CommentsThreadPort>,',
  'Self {\n            db,\n            comments_thread_port,\n        }',
  'mod port_injection_tests',
  'fn comment_service_accepts_an_injected_comments_thread_port()',
  ') -> CommentService = CommentService::with_comments_thread_port;',
  '.comments_thread_port',
  '.create_comment(',
  '.get_comment(',
  '.list_comments_for_target(',
  '.list_public_comments_for_target(',
  '.update_comment(',
  '.set_comment_status(',
  '.delete_comment(',
  'comments_write_port_context(',
  'comments_read_port_context(',
  'comments_public_read_port_context(',
  'PortActor::service(PUBLIC_COMMENTS_PORT_ACTOR)',
  '.with_deadline(std::time::Duration::from_secs(2))',
  '.with_idempotency_key(format!("{correlation_id}:command:{command_id}"))',
  'PortErrorKind::Unavailable => rustok_core::error::ErrorKind::ExternalService',
  'PortErrorKind::Timeout => rustok_core::error::ErrorKind::Timeout',
  'BlogError::Rich(Box::new(',
  '.with_error_code(error.code)',
  'body: input.content',
  'content: record.body',
  'content_text: record.body_text',
  'Self::ensure_blog_target(&existing)?',
]) requireMarker(service, marker, servicePath);

for (const marker of [
  'pub enum BlogCommentsAvailability',
  '#[serde(rename_all = "SCREAMING_SNAKE_CASE")]',
  'Available,',
  'Unavailable,',
  'Timeout,',
  'pub availability: BlogCommentsAvailability',
]) requireMarker(storefrontModel, marker, storefrontModelPath);

for (const marker of [
  'fn comments_read_availability(',
  'rustok_core::error::ErrorKind::ExternalService',
  'Some(BlogCommentsAvailability::Unavailable)',
  'rustok_core::error::ErrorKind::Timeout',
  'Some(BlogCommentsAvailability::Timeout)',
  'let Some(availability) = comments_read_availability(&error) else',
  'return Err(ServerFnError::new(error));',
  'availability: BlogCommentsAvailability::Available',
  'items: Vec::new()',
  'total: 0',
]) requireMarker(storefrontNative, marker, storefrontNativePath);
requireNoMarker(storefrontNative, 'Err(_) => BlogCommentList', storefrontNativePath);

for (const marker of [
  'pub enum GqlBlogCommentsAvailability',
  'pub availability: GqlBlogCommentsAvailability',
  'fn graphql_comments_read_availability(error: &BlogError)',
  'ErrorKind::ExternalService => Some(GqlBlogCommentsAvailability::Unavailable)',
  'ErrorKind::Timeout => Some(GqlBlogCommentsAvailability::Timeout)',
  'let Some(availability) = graphql_comments_read_availability(&error) else',
  'return Err(async_graphql::Error::new(error.to_string()));',
  'GqlBlogCommentsAvailability::Available',
]) requireMarker(graphqlOwner, marker, graphqlOwnerPath);
requireNoMarker(graphqlOwner, 'Err(_) => (Vec::new(), 0', graphqlOwnerPath);

requireMarker(
  storefrontGraphql,
  'publicComments(locale: $locale, page: $commentsPage, perPage: $commentsPerPage) { availability total items',
  storefrontGraphqlPath,
);

for (const marker of [
  'comments.availability != BlogCommentsAvailability::Available',
  'BlogCommentsAvailability::Unavailable',
  'BlogCommentsAvailability::Timeout',
  'Comments are temporarily unavailable. The article is still available.',
  'Comments took too long to load. The article is still available.',
]) requireMarker(storefrontUi, marker, storefrontUiPath);

const directBypasses = [...service.matchAll(/\.comments\s*\.\s*([a-z_]+)\s*\(/g)].map((match) => match[1]);
if (directBypasses.length > 0) {
  failures.push(`${servicePath}: direct CommentsService bypass ${directBypasses.sort().join('|')}`);
}

for (const smokeCase of fallbackEvidence?.fallback_smoke?.cases ?? []) {
  for (const marker of smokeCase.source_markers ?? []) requireMarker(service, marker, `${fallbackEvidencePath}:${smokeCase.operation}`);
  for (const marker of smokeCase.typed_error_markers ?? []) requireMarker(service, marker, `${fallbackEvidencePath}:${smokeCase.operation}:error`);
  if (!expectedOperations.includes(smokeCase.operation)) failures.push(`${fallbackEvidencePath}: unknown operation ${smokeCase.operation}`);
  if (!(evidence?.fallback_smoke?.degraded_modes ?? []).includes(smokeCase.degraded_mode)) {
    failures.push(`${fallbackEvidencePath}: degraded mode drift for ${smokeCase.operation}`);
  }
}

for (const marker of [
  'blog-comments-consumer-static-matrix.json',
  'blog-comments-runtime-fallback-smoke.json',
  'verify:blog:comments-port-boundary',
  'test:verify:blog:comments-port-boundary',
  'source_verified_no_compile',
  'typed storefront comments availability',
  'CommentService::with_comments_thread_port',
  'remote transport remains pending',
  'cached snapshot and comment-form fallback remain planned',
]) requireMarker(plan, marker, planPath);

if (failures.length > 0) {
  console.error('Blog Comments port boundary verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Blog Comments consumer port, injection seam, and storefront read-degradation source boundary is consistent');
