import fs from 'node:fs';

const evidencePath = 'crates/rustok-blog/contracts/evidence/blog-comments-host-provider-selection.json';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-70.md';
const selectorPath = 'apps/server/src/services/comments_provider_runtime.rs';
const servicesPath = 'apps/server/src/services/mod.rs';
const distributionPath = 'crates/rustok-distribution/Cargo.toml';
const graphqlConsumerPath = 'crates/rustok-blog/src/graphql/runtime_data.rs';
const httpConsumerPath = 'crates/rustok-blog/src/controllers/mod.rs';
const graphqlHostPath = 'apps/server/src/services/graphql_schema.rs';
const nativeHostPath = 'apps/server/src/services/app_router.rs';

function read(path) {
  return fs.readFileSync(path, 'utf8');
}

function fail(message) {
  console.error(`[verify-blog-comments-host-provider-selection] ${message}`);
  process.exit(1);
}

function hasAll(text, snippets, label) {
  for (const snippet of snippets) {
    if (!text.includes(snippet)) fail(`${label} missing ${snippet}`);
  }
}

function hasNone(text, snippets, label) {
  for (const snippet of snippets) {
    if (text.includes(snippet)) fail(`${label} contains forbidden ${snippet}`);
  }
}

for (const path of [
  evidencePath,
  planPath,
  selectorPath,
  servicesPath,
  distributionPath,
  graphqlConsumerPath,
  httpConsumerPath,
  graphqlHostPath,
  nativeHostPath,
]) {
  if (!fs.existsSync(path)) fail(`missing source artifact ${path}`);
}

const evidence = JSON.parse(read(evidencePath));
const plan = read(planPath);
const selector = read(selectorPath);
const services = read(servicesPath);
const distribution = read(distributionPath);
const graphqlConsumer = read(graphqlConsumerPath);
const httpConsumer = read(httpConsumerPath);
const graphqlHost = read(graphqlHostPath);
const nativeHost = read(nativeHostPath);

if (evidence.schema_version !== 1) fail('evidence schema_version drift');
if (
  evidence.module !== 'blog'
  || evidence.provider !== 'comments'
  || evidence.surface !== 'comments_host_provider_selection'
) fail('evidence identity drift');
if (
  evidence.status !== 'source_verified_no_compile'
  || evidence.compile_policy !== 'not_run_by_request'
  || evidence.runtime_status !== 'not_run'
) fail('evidence execution status drift');
if (evidence.generated_from !== planPath) fail('evidence plan path drift');

if (
  evidence.configuration?.mode_environment !== 'RUSTOK_COMMENTS_PROVIDER_MODE'
  || evidence.configuration?.endpoint_environment !== 'RUSTOK_COMMENTS_TCP_ENDPOINT'
  || evidence.configuration?.default_mode !== 'in_process'
  || evidence.configuration?.tcp_loopback_required !== true
  || evidence.configuration?.silent_tcp_fallback !== false
) fail('configuration evidence drift');

if (
  evidence.publication?.selected_value !== 'Arc<dyn rustok_comments::CommentsThreadPort>'
  || evidence.publication?.graphql !== true
  || evidence.publication?.axum_http !== true
  || evidence.publication?.server_functions !== true
  || evidence.publication?.separate_transport_specific_consumer_wiring !== false
) fail('publication evidence drift');

hasAll(
  selector,
  [
    'pub const COMMENTS_PROVIDER_MODE_ENV: &str = "RUSTOK_COMMENTS_PROVIDER_MODE";',
    'pub const COMMENTS_TCP_ENDPOINT_ENV: &str = "RUSTOK_COMMENTS_TCP_ENDPOINT";',
    'pub enum CommentsProviderProfile',
    'InProcessFallback',
    'Preconfigured',
    'TcpLoopback',
    'extensions.contains::<Arc<dyn CommentsThreadPort>>()',
    '"in_process"',
    '"tcp"',
    'raw_endpoint.trim().parse::<SocketAddr>()',
    'endpoint.ip().is_loopback()',
    'TcpJsonCommentsTransport::new(endpoint)',
    'remote_comments_thread_port(transport)',
    'extensions.insert::<Arc<dyn CommentsThreadPort>>',
    'RUSTOK_COMMENTS_PROVIDER_MODE} must be one of: in_process, tcp',
  ],
  'server Comments selector',
);

hasNone(
  selector,
  [
    'TcpStream::connect',
    'ToSocketAddrs',
    'lookup_host',
    'unwrap_or_else(|_| "tcp"',
    '0.0.0.0:',
    'retry(',
  ],
  'selector non-claims',
);

hasAll(
  services,
  [
    '#[cfg(feature = "mod-comments")]',
    'pub mod comments_provider_runtime;',
    'register_comments_provider_runtime(&mut extensions)',
    '.map_err(Error::BadRequest)?;',
    'Ok(Arc::new(extensions))',
  ],
  'host extension composition',
);

hasAll(
  distribution,
  [
    'mod-comments = ["dep:rustok-comments", "rustok-comments/tcp-transport", "rustok-content-orchestration/mod-comments"]',
  ],
  'distribution feature wiring',
);

hasAll(
  graphqlConsumer,
  [
    'inputs.shared_get::<Arc<dyn CommentsThreadPort>>()',
    'CommentService::with_comments_thread_port',
    'None => CommentService::new(db, event_bus)',
  ],
  'Blog GraphQL consumer',
);

hasAll(
  httpConsumer,
  [
    'runtime.shared_get::<Arc<dyn CommentsThreadPort>>()',
    'CommentService::with_comments_thread_port',
    'CommentService::new(self.db_clone(), self.event_bus())',
  ],
  'Blog HTTP consumer',
);

hasAll(
  graphqlHost,
  [
    'runtime_extensions.apply_to_host_runtime(host_runtime)',
    'GraphqlRuntimeInputs::new(host_runtime)',
  ],
  'GraphQL host snapshot',
);

hasAll(
  nativeHost,
  [
    'extensions.apply_to_host_runtime(runtime_ctx)',
    'append_optional_module_axum_routers(router, &server_fn_runtime_ctx)',
    'provide_context(runtime_ctx.clone())',
  ],
  'native and HTTP host snapshot',
);

hasAll(
  plan,
  [
    '# rustok-blog implementation plan — slice 70 continuation',
    'RUSTOK_COMMENTS_PROVIDER_MODE',
    'RUSTOK_COMMENTS_TCP_ENDPOINT',
    'loopback endpoint',
    'existing database/event-bus fallback',
    'source_verified_no_compile',
    'not_run_by_request',
    'listener lifecycle',
    'retry/backoff',
  ],
  'slice 70 plan',
);

console.log('[verify-blog-comments-host-provider-selection] source contract verified');
