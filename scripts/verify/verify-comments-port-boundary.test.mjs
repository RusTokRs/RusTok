#!/usr/bin/env node

import test from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const verifier = path.resolve("scripts/verify/verify-comments-port-boundary.mjs");
const registryPath = "crates/rustok-comments/contracts/comments-fba-registry.json";
const evidencePath =
  "crates/rustok-comments/contracts/evidence/comments-contract-test-static-matrix.json";
const sharedPolicyPath = "crates/rustok-api/src/ports.rs";
const providerPath = "crates/rustok-comments/src/ports.rs";
const publicReadPath = "crates/rustok-comments/src/public_read.rs";
const dtoPath = "crates/rustok-comments/src/dto.rs";
const richtextPath = "crates/rustok-comments/src/richtext.rs";
const planPath = "crates/rustok-comments/docs/implementation-plan.md";
const verifierPath = "scripts/verify/verify-comments-port-boundary.mjs";
const selfTestPath = "scripts/verify/verify-comments-port-boundary.test.mjs";

const operations = [
  "create_comment",
  "get_comment",
  "list_comments_for_target",
  "list_public_comments_for_target",
  "update_comment",
  "set_comment_status",
  "delete_comment",
];
const writeOperations = [
  "create_comment",
  "update_comment",
  "set_comment_status",
  "delete_comment",
];
const readOperations = [
  "get_comment",
  "list_comments_for_target",
  "list_public_comments_for_target",
];

function write(root, relativePath, content) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function operationSource(operation, { missingWritePolicy, publicBypass } = {}) {
  const writeOperation = writeOperations.includes(operation);
  const policy = writeOperation
    ? missingWritePolicy && operation === "create_comment"
      ? ""
      : "context.require_policy(PortCallPolicy::write())?;"
    : "context.require_policy(PortCallPolicy::read())?;";
  if (operation === "list_public_comments_for_target") {
    return `
      async fn list_public_comments_for_target(
        &self,
        context: PortContext,
        target_type: String,
        target_id: Uuid,
        filter: ListCommentsFilter,
        fallback_locale: Option<String>,
      ) -> Result<(Vec<CommentListItem>, u64), PortError> {
        ${policy}
        ${
          publicBypass
            ? "self.list_comments_for_target(context, target_type, target_id, filter, fallback_locale).await"
            : "crate::public_read::list_public_comments_for_target(&self.db, tenant_id, &target_type, target_id, filter, fallback_locale.as_deref()).await.map_err(comments_error_to_port_error)"
        }
      }
    `;
  }
  return `
    async fn ${operation}(&self, context: PortContext) -> Result<CommentRecord, PortError> {
      ${policy}
      self.service.${operation}().await.map_err(comments_error_to_port_error)
    }
  `;
}

function fixture({
  missingOperation = false,
  missingWritePolicy = false,
  publicBypass = false,
  missingApprovedFilter = false,
  missingErrorMapping = false,
  promoteRemoteProfile = false,
  promoteRuntime = false,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-comments-port-boundary-"));
  const fixtureOperations = missingOperation
    ? operations.filter((operation) => operation !== "get_comment")
    : operations;
  const sourceProfiles = promoteRemoteProfile
    ? ["in_process", "remote_adapter_placeholder"]
    : ["in_process"];
  const pendingProfiles = promoteRemoteProfile ? [] : ["remote_adapter_placeholder"];
  const runtimeStatus = promoteRuntime ? "executed" : "pending";

  write(
    root,
    sharedPolicyPath,
    `
      pub struct PortCallPolicy { requires_deadline: bool, requires_idempotency_key: bool }
      impl PortCallPolicy {
        pub const fn read() -> Self { Self { requires_deadline: true, requires_idempotency_key: false } }
        pub const fn write() -> Self { Self { requires_deadline: true, requires_idempotency_key: true } }
      }
      fn require_policy() { self.require_write_semantics(); self.require_read_semantics(); }
    `,
  );

  write(
    root,
    providerPath,
    `
      pub trait CommentsThreadPort: Send + Sync {
        ${fixtureOperations.map((operation) => `async fn ${operation}(&self);`).join("\n")}
      }
      struct InProcessCommentsThreadProvider { db: DatabaseConnection, service: CommentsService }
      pub fn in_process_comments_thread_port() {
        CommentsService::with_event_bus(db.clone(), event_bus);
      }
      impl CommentsThreadPort for InProcessCommentsThreadProvider {
        ${fixtureOperations
          .map((operation) =>
            operationSource(operation, { missingWritePolicy, publicBypass }),
          )
          .join("\n")}
      }
      fn parse_tenant_id(context: &PortContext) {}
      fn comments_error_to_port_error(error: CommentsError) {
        CommentsError::Database(source);
        ${missingErrorMapping ? "" : 'PortError::unavailable("comments.database", source.to_string());'}
        CommentsError::EventPublication(message);
        PortErrorKind::NotFound;
        PortErrorKind::Conflict;
        PortErrorKind::Forbidden;
        PortError::validation("comments.validation", message);
      }
    `,
  );

  write(
    root,
    publicReadPath,
    `
      comment_thread::Column::TenantId.eq(tenant_id);
      comment_thread::Column::TargetType.eq(target_type);
      comment_thread::Column::TargetId.eq(target_id);
      comment::Column::TenantId.eq(tenant_id);
      comment::Column::DeletedAt.is_null();
      ${missingApprovedFilter ? "" : "comment::Column::Status.eq(CommentStatus::Approved);"}
      filter.per_page.clamp(1, MAX_PUBLIC_COMMENTS_PER_PAGE);
      project_comment_body(&resolved.body)?;
      projection.plain_text.chars().take(200);
    `,
  );

  write(
    root,
    dtoPath,
    `
      pub body: RichTextDocument
      pub body: Option<RichTextDocument>
      pub body: RichTextView
      pub body_text: String
      pub body_preview: String
    `,
  );

  write(
    root,
    richtextPath,
    `
      serialize_comment_body(document: RichTextDocument)
      validate_and_normalize(document, RichTextProfile::Comment)
      project_comment_body(raw: &str)
      parse_json(raw, RichTextProfile::Comment)
      plain_text(&document, RichTextProfile::Comment)
      project(&document, RichTextProfile::Comment)
    `,
  );

  const cases = operations.map((operation) => ({
    operation,
    assertions: ["typed_port_error_mapping", "context_deadline_preserved"],
    runtime_evidence: "pending",
  }));
  const fallbackSmoke = {
    status: "planned",
    profiles: ["embedded_native"],
    degraded_modes: ["hide_comment_form", "show_cached_thread_snapshot"],
    runtime_evidence: "pending",
  };

  write(
    root,
    evidencePath,
    JSON.stringify({
      schema_version: 2,
      module: "comments",
      surface: "comments_thread_port_boundary",
      role: "provider",
      generated_from: registryPath,
      status: "source_verified_no_compile",
      compile_policy: "not_run_by_request",
      runtime_status: runtimeStatus,
      source_contract: {
        shared_port_policy: sharedPolicyPath,
        provider_port: providerPath,
        public_projection: publicReadPath,
        dto: dtoPath,
        richtext: richtextPath,
        provider_registry: registryPath,
      },
      profiles: {
        source_verified: sourceProfiles,
        pending: pendingProfiles,
      },
      cases,
      fallback_smoke: fallbackSmoke,
    }),
  );

  write(
    root,
    registryPath,
    JSON.stringify({
      schema_version: 4,
      module: "comments",
      role: "provider",
      status: "boundary_ready",
      contract_version: "comments.thread.v1",
      ports: [
        {
          name: "CommentsThreadPort",
          operations,
          write_operations: writeOperations,
          read_operations: readOperations,
        },
      ],
      verification_chain: {
        source_gates: {
          comments_port_boundary: {
            package_script: "verify:comments:port-boundary",
            test_package_script: "test:verify:comments:port-boundary",
            verifier: verifierPath,
            self_test: selfTestPath,
            evidence: evidencePath,
          },
        },
      },
      contract_tests: {
        status: "source_verified_no_compile",
        runtime_status: runtimeStatus,
        runner: verifierPath,
        source_profiles: sourceProfiles,
        pending_profiles: pendingProfiles,
        cases,
        fallback_smoke: fallbackSmoke,
      },
    }),
  );

  write(
    root,
    "package.json",
    JSON.stringify({
      scripts: {
        "verify:comments:port-boundary": `node ${verifierPath}`,
        "test:verify:comments:port-boundary": `node ${selfTestPath}`,
      },
    }),
  );
  write(
    root,
    planPath,
    "comments-contract-test-static-matrix.json verify:comments:port-boundary test:verify:comments:port-boundary source_verified_no_compile remote adapter remains pending",
  );

  return root;
}

function run(root) {
  return spawnSync(process.execPath, [verifier], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

function expectRejected(options, pattern) {
  const root = fixture(options);
  try {
    const result = run(root);
    assert.notEqual(result.status, 0, result.stdout);
    if (pattern) assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("accepts the Comments in-process provider source boundary", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("rejects a missing provider operation", () => {
  expectRejected({ missingOperation: true }, /missing async fn get_comment/);
});

test("rejects a write operation without shared policy", () => {
  expectRejected({ missingWritePolicy: true }, /create_comment.*missing context\.require_policy/);
});

test("rejects public reads that delegate to the authenticated list", () => {
  expectRejected({ publicBypass: true }, /public projection/);
});

test("rejects public projection without approved-only filtering", () => {
  expectRejected({ missingApprovedFilter: true }, /CommentStatus::Approved/);
});

test("rejects missing typed database error mapping", () => {
  expectRejected({ missingErrorMapping: true }, /comments\.database/);
});

test("rejects source promotion of the remote placeholder", () => {
  expectRejected({ promoteRemoteProfile: true }, /source-verified profile drift|source profile drift/);
});

test("rejects runtime promotion without retained execution", () => {
  expectRejected({ promoteRuntime: true }, /execution policy drift|contract-test status\/runner drift/);
});
