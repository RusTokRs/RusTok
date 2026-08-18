#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? path.resolve(configuredRoot)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const read = (relativePath) => readFileSync(path.join(root, relativePath), "utf8");
const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireBefore = (source, first, second, label) => {
  const firstIndex = source.indexOf(first);
  const secondIndex = source.indexOf(second);
  if (firstIndex < 0 || secondIndex < 0 || firstIndex >= secondIndex) {
    failures.push(`${label}: expected ${first} before ${second}`);
  }
};

const files = {
  event: "crates/rustok-events/src/rbac_role_mutation.rs",
  eventContract: "crates/rustok-events/src/contract.rs",
  eventRegistry: "crates/rustok-events/src/lib.rs",
  owner: "crates/rustok-rbac/src/role_mutation.rs",
  ownerLib: "crates/rustok-rbac/src/lib.rs",
  adapter:
    "apps/server/src/services/auth_admin_mutation_provider/user_admin.rs",
  continuity:
    "apps/server/src/services/auth_admin_mutation_provider/super_admin_guard.rs",
  graphql: "apps/server/src/graphql/rbac_runtime.rs",
  machine: "crates/rustok-rbac/contracts/rbac-owner-role-mutation-contract.json",
  docs: "crates/rustok-rbac/docs/owner-role-mutation-contract.md",
  plan: "crates/rustok-rbac/docs/implementation-plan.md",
  master: "docs/verification/PLATFORM_VERIFICATION_PLAN.md",
};
const sources = Object.fromEntries(
  Object.entries(files).map(([name, relativePath]) => [name, read(relativePath)]),
);

for (const marker of [
  'pub const RBAC_EVENT_USER_ROLE_REPLACED: &str = "rbac.user_role_replaced"',
  '"rbac.user_role_assignment_repaired"',
  "pub enum RbacRoleMutationEvent",
  "UserRoleReplaced {",
  "previous_role: String",
  "new_role: String",
  "UserRoleAssignmentRepaired {",
  "durable_generation: u64",
  "impl sealed::Sealed for RbacRoleMutationEvent",
  "impl EventContract for RbacRoleMutationEvent",
  "ContractEventPayload::RbacRoleMutation(self)",
  "validators::validate_not_nil_uuid(\"user_id\", user_id)",
  'matches!(value, "super_admin" | "admin" | "manager" | "customer")',
  "if generation == 0",
  "RBAC_ROLE_MUTATION_EVENT_SCHEMAS",
]) requireText(sources.event, marker, `${files.event}: typed event family`);

for (const marker of [
  "RbacRoleMutationEvent",
  '#[serde(rename = "rbac_role_mutation")]\n    RbacRoleMutation(RbacRoleMutationEvent)',
  "Self::RbacRoleMutation(event) => event.event_type()",
  "Self::RbacRoleMutation(event) => event.schema_version()",
  "Self::RbacRoleMutation(event) => event.validate()",
]) requireText(
  sources.eventContract,
  marker,
  `${files.eventContract}: sealed payload registration`,
);

for (const marker of [
  "mod rbac_role_mutation;",
  "RbacRoleMutationEvent",
  "rbac_role_mutation_event_schema(event_type)",
  ".chain(RBAC_ROLE_MUTATION_EVENT_SCHEMAS.iter())",
]) requireText(
  sources.eventRegistry,
  marker,
  `${files.eventRegistry}: schema registry`,
);

for (const marker of [
  "pub struct RbacRoleMutationFacts",
  "pub enum RbacRoleMutationOutcome",
  "pub struct RbacRoleMutationPlan",
  "pub enum RbacRoleMutationChange",
  "ActorTenantMismatch",
  "TargetTenantMismatch",
  "CannotAssignPeerOrHigherRole",
  "CannotManagePeerOrHigherUser",
  "LastActiveSuperAdmin",
  "pub fn plan_user_role_mutation",
  "facts.actor_tenant_id != facts.tenant_id",
  "facts.target_tenant_id != facts.tenant_id",
  "facts.actor_role.can_assign_role(&facts.requested_role)",
  "facts.actor_role.can_manage_role(&facts.target_role)",
  "facts.remaining_active_super_admins == 0",
  "facts.assignment_is_exact && facts.target_role == facts.requested_role",
  "RbacRoleMutationChange::AssignmentRepaired",
  "RbacRoleMutationChange::RoleReplaced",
  "pub fn integration_event",
  "durable_generation: u64",
]) requireText(sources.owner, marker, `${files.owner}: owner policy`);

for (const marker of [
  "mod role_mutation;",
  "RbacRoleMutationFacts",
  "RbacRoleMutationPolicyError",
  "plan_user_role_mutation",
]) requireText(sources.ownerLib, marker, `${files.ownerLib}: public owner boundary`);

const updateUserStart = sources.adapter.indexOf("async fn update_user(");
const deleteUserStart = sources.adapter.indexOf("async fn delete_user(");
if (updateUserStart < 0 || deleteUserStart <= updateUserStart) {
  failures.push(`${files.adapter}: update_user body could not be isolated`);
}
const updateUser = sources.adapter.slice(updateUserStart, deleteUserStart);
for (const marker of [
  "RbacRoleMutationFacts",
  "plan_user_role_mutation",
  "RbacRoleMutationOutcome::Noop",
  "RbacRoleMutationOutcome::Apply(plan)",
  "count_remaining_active_super_admins",
  "has_exact_tenant_role_assignment",
  "RbacService::replace_user_role_in_transaction",
  "reserve_rbac_invalidation_generation(&tx)",
  ".integration_event(generation)",
  ".publish_contract_in_tx(",
  "tx.commit()",
  "publish_committed_user_invalidation",
  "durable RBAC role mutation event is unavailable",
]) requireText(updateUser, marker, `${files.adapter}: composed mutation path`);
requireBefore(
  updateUser,
  "RbacService::replace_user_role_in_transaction",
  "reserve_rbac_invalidation_generation(&tx)",
  `${files.adapter}: relation before generation`,
);
requireBefore(
  updateUser,
  "reserve_rbac_invalidation_generation(&tx)",
  ".integration_event(generation)",
  `${files.adapter}: generation before event construction`,
);
requireBefore(
  updateUser,
  ".publish_contract_in_tx(",
  "tx.commit()",
  `${files.adapter}: outbox before commit`,
);
requireBefore(
  updateUser,
  "tx.commit()",
  "publish_committed_user_invalidation",
  `${files.adapter}: commit before cache fan-out`,
);
for (const forbidden of [
  "can_assign_role(requested_role)",
  "can_manage_role(&current_role)",
  "INSERT INTO sys_outbox",
  "INSERT INTO outbox",
  "rbac.user_role_replaced\".to_string()",
  "publish_user_rbac_invalidation(&context.tenant_id",
]) forbidText(updateUser, forbidden, `${files.adapter}: forbidden host shortcut`);

for (const marker of [
  "pub(super) async fn count_remaining_active_super_admins",
  "lock_super_admin_role(db, tenant_id)",
  "filter(users::Column::Id.ne(target_user_id))",
  "filter(users::Column::Status.eq(UserStatus::Active))",
]) requireText(sources.continuity, marker, `${files.continuity}: locked continuity fact`);

for (const marker of [
  "ServerRbacGraphqlRoleWriter",
  "UserAdminMutationRuntime",
  ".update_user(",
]) requireText(sources.graphql, marker, `${files.graphql}: existing transport facade`);
for (const forbidden of [
  "replace_user_role_in_transaction",
  "user_roles::Entity",
  'route("/roles"',
]) forbidText(sources.graphql, forbidden, `${files.graphql}: no parallel role transport`);

const machine = JSON.parse(sources.machine);
const checks = [
  [machine.status === "source_ready_unvalidated", "status must remain source_ready_unvalidated"],
  [machine.base_revision === "6ca587546e5a218d38c49f7c0612edcf61d8f816", "base revision must remain exact"],
  [machine.cycle === "cycle-001", "cycle must remain cycle-001"],
  [machine.component === "core/rbac", "component must remain core/rbac"],
  [machine.priority === "P1", "priority must remain P1"],
  [machine.owner?.policy === "plan_user_role_mutation", "owner policy must remain canonical"],
  [machine.persistence?.atomic_transaction === true, "mutation and outbox must remain atomic"],
  [machine.persistence?.event_failure_rolls_back_relation_user_and_generation === true, "event failure must roll back owner state"],
  [machine.transport?.new_rest_surface === false, "new REST surface must remain absent"],
  [machine.transport?.new_graphql_surface === false, "new GraphQL surface must remain absent"],
  [machine.transport?.new_native_surface === false, "new native surface must remain absent"],
  [machine.validation?.rust_tests_executed === false, "Rust execution must not be claimed"],
  [machine.validation?.source_verifier_executed === false, "verifier execution must not be claimed"],
  [machine.validation?.cargo_checked === false, "Cargo validation must not be claimed"],
  [machine.validation?.outbox_runtime_executed === false, "outbox runtime execution must not be claimed"],
  [machine.remaining_gates?.custom_role_and_permission_mutation_contract === false, "permission/custom-role gate must remain open"],
  [machine.remaining_gates?.native_operator_parity === false, "native parity gate must remain open"],
  [machine.remaining_gates?.core_rbac_complete === false, "RBAC must remain incomplete"],
  [machine.cursor_advanced === false, "cursor must not advance"],
];
for (const [passed, message] of checks) {
  if (!passed) failures.push(`${files.machine}: ${message}`);
}

for (const marker of [
  "Status: `source_ready_unvalidated`",
  "plan_user_role_mutation",
  "AssignmentRepaired",
  "RoleReplaced",
  "TransactionalEventBus::publish_contract_in_tx",
  "rbac.user_role_replaced",
  "rbac.user_role_assignment_repaired",
  "No `/roles` endpoint",
  "custom role creation",
  "did not run Rust tests",
]) requireText(sources.docs, marker, `${files.docs}: documented boundary`);

for (const marker of [
  "### P1 — operator parity and lifecycle",
  "- [ ] Define custom-role and arbitrary permission mutation ownership.",
  "- [ ] Route native operator management through owner policy without parallel writes.",
  "- [ ] Identify idempotent, non-authoritative event consumers.",
]) requireText(sources.plan, marker, `${files.plan}: broader P1 remains open`);
for (const marker of [
  "Current item: `core/rbac`",
  "Next item: `core/rbac`",
  "Release readiness: `not_assessed`",
]) requireText(sources.master, marker, `${files.master}: active cursor`);

if (failures.length > 0) {
  console.error("RBAC owner role mutation contract source verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ source-ready RBAC owner role mutation policy governs built-in role hierarchy, tenant scope, continuity, exact no-op and malformed-assignment repair while the existing user-admin facade writes a typed event with the same durable generation in one transaction and all wider execution and permission-management gates remain open",
);
