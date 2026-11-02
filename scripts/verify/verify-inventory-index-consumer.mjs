#!/usr/bin/env node
// Read-only, supplementary source obligations. No Rust/test/SQL/runtime execution.
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { fileURLToPath } from "node:url";
import { containsMarker, spaced } from "./lib/source-text.mjs";
import { rustFunctionBody } from "./verify-cart-priced-write-context.mjs";
import { verifyIndexSourceRefreshEvent } from "./verify-index-source-refresh-event.mjs";

const repository = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
const bridge = "crates/modules/rustok-distribution/src/inventory_index";
const index = "crates/modules/rustok-index";
export const files = {
  selection: `${bridge}/mod.rs`,
  schema: `${bridge}/schema.rs`,
  source: `${bridge}/source.rs`,
  consumer: `${bridge}/consumer.rs`,
  regression: `${bridge}/tests.rs`,
  distribution: "crates/modules/rustok-distribution/src/lib.rs",
  inventoryModule: "crates/modules/rustok-inventory/src/module.rs",
  auth: "crates/modules/rustok-auth/src/worker_context.rs",
  provider: "apps/server/src/services/worker_service_context.rs",
  secrets: "crates/utils/rustok-secrets/src/lib.rs",
  principal: "crates/libs/rustok-api/src/context/principal.rs",
  host: "apps/server/src/services/module_event_dispatcher.rs",
  lifecycle: `${index}/src/module.rs`,
  factories: `${index}/src/infrastructure/postgres/source_factory.rs`,
  enqueue: `${index}/src/infrastructure/postgres/source_replay_enqueue.rs`,
  jobs: `${index}/src/infrastructure/postgres/source_replay_job.rs`,
  runner: `${index}/src/infrastructure/postgres/source_replay_runner.rs`,
  scheduler: `${index}/src/infrastructure/postgres/source_replay_scheduler.rs`,
  workerRegression: `${index}/src/infrastructure/postgres/source_replay_scheduler_tests.rs`,
  jobRegression: `${index}/src/infrastructure/postgres/source_replay_job_tests.rs`,
  runtime: `${index}/src/infrastructure/postgres/replay_runtime.rs`,
  refresh: `${index}/src/application/source_refresh_event.rs`,
  mutationStore: `${index}/src/infrastructure/postgres/mutation_store.rs`,
  refreshRegression: `${index}/src/application/source_refresh_event_tests.rs`,
  application: `${index}/src/application/mod.rs`,
  publicApi: `${index}/src/lib.rs`,
  refreshDocs: `${index}/docs/m5-source-refresh-event.md`,
  docs: "crates/modules/rustok-distribution/docs/inventory-index.md",
  metadata:
    "crates/modules/rustok-inventory/contracts/inventory-fba-registry.json",
  packet:
    "crates/modules/rustok-inventory/contracts/evidence/source-index-consumer-source.json",
  package: "package.json",
};

const production = (source) =>
  source
    .replace(
      /#\[cfg\(test\)\]\s*(?:#\[path\s*=\s*"[^"]+"\]\s*)?mod\s+\w+\s*;/g,
      "",
    )
    .split(/#\[cfg\(test\)\]/)[0]
    .replace(/^\s*\/\/.*$/gm, "");
export function verifyInventoryIndexConsumer({ root = repository } = {}) {
  const failures = [];
  const read = (name) => {
    try {
      return fs.readFileSync(path.join(root, name), "utf8");
    } catch {
      failures.push(`missing ${name}`);
      return "";
    }
  };
  const all = Object.fromEntries(
    Object.entries(files).map(([key, name]) => [key, read(name)]),
  );
  const code = Object.fromEntries(
    Object.entries(all).map(([key, source]) => [key, production(source)]),
  );
  const require = (source, marker, label) => {
    if (!containsMarker(source, marker))
      failures.push(`${label}: missing ${marker}`);
  };
  const forbid = (source, marker, label) => {
    if (containsMarker(source, marker))
      failures.push(`${label}: forbidden ${marker}`);
  };
  const body = (source, name) => {
    const value = rustFunctionBody(source, name);
    if (!value) failures.push(`missing production body ${name}`);
    return value;
  };
  const ordered = (source, markers, label) => {
    let previous = -1;
    const flat = spaced(source);
    for (const marker of markers) {
      const at = flat.indexOf(spaced(marker));
      if (at <= previous)
        failures.push(`${label}: wrong order/missing ${marker}`);
      previous = at;
    }
  };
  require(code.distribution, "inventory_index::register", "actual Distribution composition");
  require(code.inventoryModule, "extensions.insert(InventoryRuntimeSelected)", "owning Inventory selection");
  require(body(
    code.selection,
    "register",
  ), "extensions.contains::<rustok_inventory::InventoryRuntimeSelected>()", "actual selected module gate");
  for (const marker of [
    "register_index_schema_source",
    "register_postgres_index_source_factory",
    "register_index_mutation_event",
    '"inventory-stock-primary"',
    "InventoryStockEventHandler::new(context)",
  ]) {
    require(code.selection, marker, "selected schema/source/listener");
  }
  require(code.lifecycle, "factories.register_event_listeners(registry, context)", "ordinary Index listener lifecycle");
  require(code.factories, "register_event_listeners(registry, context)", "existing factory listener seam");
  for (const marker of [
    '"rustok-inventory"',
    '"stock"',
    "LocaleMode::None",
    '"available"',
    '"inventory_item_id"',
  ])
    require(code.schema, marker, "owner-neutral schema projection");

  const authority = body(code.source, "authenticated_baseline");
  for (const marker of [
    "WorkerServiceContextRequest::new",
    ".current_context(request)",
    ".baseline_status(&context)",
    "baseline.active",
  ])
    require(authority, marker, "current authoritative owner read");
  for (const marker of [
    "PortActor::system",
    "PortActor::service",
    "UserRole::",
    "with_claim(",
    "with_idempotency_key(",
    ".actor_id()",
    "SELECT ",
    "INSERT INTO",
    "UPDATE inventory_",
  ]) {
    forbid(
      code.source + code.consumer,
      marker,
      "no fabricated authority/source SQL",
    );
  }
  for (const marker of [
    "resolve_current_user_from_access_token",
    "resolve_fresh_for_tenant",
    "permission_availability_for",
    "AuthPrincipalKind::Service",
    '"client_credentials"',
    "auth.port_actor()",
    "deserialize_secret_reference",
    "deny_unknown_fields",
  ])
    require(code.provider, marker, "canonical current service authentication");
  forbid(code.provider, "INVENTORY_READ", "host never adds consumer grants");
  forbid(
    code.provider,
    "PortActor::service(",
    "worker actor follows canonical service principal mapping",
  );
  const nativeContext = body(code.provider, "current_port_context");
  for (const marker of [
    "AuthContext {",
    "user_id: current.user.id",
    "client_id: Some(client_id)",
    "auth.port_actor()",
  ])
    require(nativeContext, marker, "worker actor follows canonical service principal mapping");
  require(body(
    code.principal,
    "port_actor",
  ), "PortActor::service(self.client_id.unwrap_or(self.user_id).to_string())", "current canonical service identity semantics");
  forbid(
    code.provider,
    ".invalidate(",
    "credential reads must bypass the cache, not invalidate then race a stale fill",
  );
  const freshSecret = body(code.secrets, "resolve_fresh_for_tenant");
  ordered(
    freshSecret,
    ["validate_reference_for_tenant", "registration.resolver.resolve"],
    "fresh resolution retains canonical tenant policy",
  );
  forbid(
    freshSecret,
    "self.cache",
    "fresh credential resolution must not touch the registry cache",
  );
  forbid(
    freshSecret,
    "resolve_for_tenant(",
    "fresh credential resolution must not delegate to the cached path",
  );
  require(code.host, "materialize_worker_service_context", "actual host provider composition");
  for (const marker of [
    "self.0.current_context(request.clone())",
    "request.validate_context(&context)",
    "PortActorKind::Service",
    "PortCallPolicy::read()",
    "context.idempotency_key.is_some()",
    "Duration::from_secs(30)",
  ])
    require(code.auth, marker, "Auth current-context contract");

  const projection = body(code.source, "mutation");
  for (const marker of [
    "row.tenant_id != tenant",
    "row.baseline_id != baseline",
    "row.source_version == 0",
    "row.stocked.checked_sub(row.reserved)",
    "event_id: row.change_id",
    "IndexMutation::Delete",
    "IndexMutation::Upsert",
    "IndexValue::Integer(row.available)",
  ])
    require(projection, marker, "retained owner identity and stock arithmetic");
  forbid(projection, "Uuid::new_v4", "stable source mutation UUID");
  for (const marker of [
    "MAX_SOURCE_PAGE",
    "READ_BUDGET",
    "Duration::from_secs(30)",
    "cursor.tenant_id != request.tenant_id()",
    "cursor.baseline_id != baseline",
    "cursor.after_variant_id.is_nil()",
    "within_budget",
  ])
    require(code.source, marker, "bounded baseline-bound native source");
  for (const marker of [
    ".changes(&context, baseline_id, variant, after, 1)",
    "change.source_version != version",
    "change.change_id != change_id",
    "change.variant_id != variant",
    "change.tenant_id != tenant",
    "change.baseline_id != baseline_id",
  ])
    require(code.source, marker, "exact committed owner fact");

  const handle = body(code.consumer, "handle_contract");
  ordered(
    handle,
    [
      "validate_registered_schema()",
      ".verify_fact(",
      "PostgresSchemaRegistrationStore::new",
    ],
    "sealed/current owner before derived persistence",
  );
  const changedStart = handle.indexOf("let delivery =");
  const activationStart = handle.indexOf(
    "let request = IndexReplayJobEnqueueRequest",
  );
  if (changedStart < 0 || activationStart <= changedStart)
    failures.push("missing actual changed/activation persistence arms");
  const changed = handle.slice(changedStart, activationStart);
  const activation = handle.slice(activationStart);
  ordered(
    changed,
    ["persist_index_source_refresh(", ".await", "Ok(())"],
    "changed persistence before handler ACK",
  );
  ordered(
    activation,
    [
      "IndexReplayJobEnqueueRequest::new(",
      ".enqueue(&request)",
      ".await",
      "Ok(())",
    ],
    "initial replay durable before handler ACK",
  );
  require(handle, "envelope.id() != baseline_id", "original activation UUID");
  require(activation, /IndexReplayJobEnqueueRequest::new\(\s*envelope\.tenant_id\(\),\s*baseline_id,/, "original initial replay job identity");
  forbid(
    code.consumer,
    "IndexMutationEventAcknowledger",
    "no dummy module acknowledger",
  );

  const enqueue = body(code.enqueue, "enqueue");
  ordered(
    body(code.enqueue, "enqueue_in_transaction"),
    [
      "lock_replay_scope",
      "super::complete_checkpoint_sql(backend)",
      "IndexReplayJobError::CheckpointWithoutJob",
      "INSERT INTO index_jobs",
    ],
    "orphan checkpoint refuses a fresh initial replay job",
  );
  ordered(
    body(code.jobs, "acquire_in_transaction"),
    [
      '"succeeded" =>',
      "require_complete_checkpoint_for_scope(transaction, request, backend)",
      "IndexReplayJobAcquireOutcome::AlreadyComplete",
    ],
    "already-complete acquisition requires durable checkpoint proof",
  );
  ordered(
    enqueue,
    ["self.enqueue_in_transaction", "transaction.commit()"],
    "transactional enqueue",
  );
  require(enqueue, /transaction\.commit\(\)[\s\S]*?\?;\s*Ok\(outcome\)/, "enqueue returns success only after commit");
  require(enqueue, "transaction.rollback()", "enqueue refuses with awaited rollback");
  for (const marker of [
    "select_replay_job_history_sql(backend, true)",
    "saved.job_id != request.job_id",
    '"cancel_requested"',
    '"failed"',
    "require_complete_checkpoint_for_scope",
  ])
    require(code.enqueue, marker, "retained original enqueue fences");
  for (const marker of [
    "Uuid::new_v4",
    "UPDATE index_jobs SET",
    "DELETE FROM index_",
    "with_claim",
  ])
    forbid(code.enqueue, marker, "no original identity/checkpoint replacement");
  require(code.jobs, "IndexReplayJobError::JobNotFound(original)", "exact fence never creates an absent job");
  require(code.jobs, "stored.job_id != expected", "exact original job acquisition");
  require(code.runner, "lease_request.for_job(job_id)", "canonical runner original job fence");
  require(code.runtime, "register_postgres_index_replay_work(extensions)", "owning runtime worker registration");
  require(code.scheduler, "shared_get::<SharedIndexReplayRuntime>()", "single canonical replay runtime");
  require(code.scheduler, ".for_job(item.id)", "worker exact original job");
  require(code.scheduler, "self.schemas.registry().iter()", "selected-only discovery");
  require(code.scheduler, "self.sources.source_for_schema(reference)", "selected-only source binding");
  require(code.scheduler, "LIMIT 1", "bounded due-job discovery");
  for (const source of [code.enqueue, code.scheduler, code.mutationStore]) {
    require(source, "CAST(schema_version AS BIGINT) AS schema_version_value", "portable stored schema integer decode");
    require(source, '.try_get("", "schema_version_value")', "portable stored schema integer decode");
    forbid(
      source,
      '.try_get("", "schema_version")',
      "no uncast PostgreSQL integer-to-i64 decode",
    );
  }

  require(code.scheduler, "Duration::from_secs(300)", "bounded renewable worker lease");
  require(code.scheduler, /128,\s*8,\s*1,/, "bounded page/record worker budget");
  require(code.scheduler, "IndexReplayRunStatus::Busy => ModuleWorkOutcome::Held", "busy is not completion");
  require(code.scheduler, "IndexReplayRunStatus::Yielded => ModuleWorkOutcome::Retryable", "yield is not job completion");
  require(code.scheduler, 'ModuleWorkOutcome::Completed => saved.state == "succeeded"', "module work checks durable job result");
  require(code.scheduler, "require_complete_checkpoint_for_scope", "module work checks durable completion checkpoint");
  forbid(
    code.scheduler,
    "PostgresIndexReplayRunner::new",
    "no second replay runtime",
  );

  try {
    failures.push(...verifyIndexSourceRefreshEvent({ root }));
  } catch (error) {
    failures.push(
      `generic refresh guard input unavailable: ${error.code ?? error.name}`,
    );
  }
  for (const marker of [
    "every_owner_invocation_reauthenticates_and_never_reuses_event_or_baseline_actor",
    "changed_persists_before_success_and_old_redelivery_keeps_advancing_owner_mutation_ids",
    "postgres_empty_activation_durably_enqueues_once_under_original_baseline_uuid",
    "postgres_changed_redelivery_keeps_one_inbox_per_owner_change_including_tombstone",
    "owner_tombstone_is_durable_and_wrong_change_or_storage_failure_withholds_success",
    "changed_refuses_unregistered_wire_version_before_authority_or_persistence",
  ])
    require(all.regression, marker, "unrun native regression definition");
  for (const marker of [
    "orphan_checkpoint_refuses_initial_enqueue_without_creating_job_or_resetting_cursor",
    "succeeded_job_acquisition_requires_complete_checkpoint_for_scoped_and_exact_requests",
    "duplicate_activation_retains_original_checkpoint_and_expired_job_attempt_fence",
    "cancelled_or_cancellation_requested_original_cannot_be_replaced_by_activation",
    "pending_enqueue_storage_failure_never_returns_success_or_leaves_partial_job",
  ])
    require(all.jobRegression, marker, "unrun enqueue regression definition");
  for (const marker of [
    "worker_yields_original_job_with_durable_checkpoint_then_finishes_without_fake_completion",
    "unselected_due_source_cannot_starve_selected_work_or_gain_an_ack",
    "owner_failure_parks_original_job_without_claiming_success_or_rotating_its_identity",
  ])
    require(all.workerRegression, marker, "unrun worker regression definition");
  for (const source of [all.regression, all.workerRegression]) {
    forbid(
      source,
      "FROM index_records",
      "regression SQL must use the canonical derived table",
    );
    require(source, "FROM index_entities", "regression SQL must use the canonical derived table");
  }
  require(all.regression, "SELECT is_deleted FROM index_entities", "regression tombstone column matches canonical storage");
  require(all.docs, "Failed-job retry/recovery is not wired", "explicit pending recovery boundary");
  require(all.docs, "RUSTOK_WORKER_SERVICE_TOKENS_JSON", "actual credential setup documentation");
  require(all.metadata, "source-index-consumer-source.json", "owner source evidence link");
  require(all.package, '"verify:inventory:index-consumer"', "focused direct verifier entry");
  try {
    const packet = JSON.parse(all.packet);
    if (
      packet.source_wiring?.source_name !== "inventory-stock-primary" ||
      packet.source_wiring?.listener_source_selected !== true
    )
      failures.push(
        "evidence must distinguish authored actual selection from deployment verification",
      );
    for (const key of [
      "compiled",
      "tests_executed",
      "sql_executed",
      "migrations_executed",
      "runtime_verified",
      "deployment_authority_verified",
      "initial_replay_durable_verified",
      "index_persist_before_ack_verified",
      "automatic_failed_job_recovery_wired",
      "index_ready",
      "step8_complete",
    ]) {
      if (packet[key] !== false)
        failures.push(`source-only evidence cannot promote ${key}`);
    }
    for (const [name, expected] of Object.entries(
      packet.preserved_hashes ?? {},
    )) {
      let actual;
      try {
        actual = crypto
          .createHash("sha256")
          .update(fs.readFileSync(path.join(root, name)))
          .digest("hex");
      } catch {
        failures.push(`missing preserved file ${name}`);
        continue;
      }
      if (actual !== expected)
        failures.push(`historical preserved hash drift: ${name}`);
    }
  } catch {
    failures.push("missing/invalid source evidence packet");
  }
  return failures;
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const failures = verifyInventoryIndexConsumer();
  if (failures.length) {
    console.error(
      failures
        .map((failure) => `[verify-inventory-index-consumer] ${failure}`)
        .join("\n"),
    );
    process.exitCode = 1;
  } else
    console.log(
      "[verify-inventory-index-consumer] source contract only; no tests, compilation, backend or runtime evidence",
    );
}
