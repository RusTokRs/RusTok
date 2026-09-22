# Index implementation recheck — finding lifecycle commands

Audited baseline: `main@d0f1aa543de2509b3b3c108c97cb4a7573eba136`.

Branch: `agent/index-m6-finding-lifecycle-20260806`.

Status: `source_complete_repair_pending`.

## Rechecked source boundaries

The lifecycle slice introduces one database-neutral command/service boundary, one PostgreSQL store,
and one Index-owned audit migration.

The command is exact and bounded:

- non-nil tenant, finding, and command UUIDs;
- resolve or ignore action only;
- explicit expected open state only;
- bounded actor kind and actor subject;
- bounded nonempty reason;
- no caller-selected target state, SQL, JSON, digest, finding key, timestamp, or repair payload.

`IndexDriftFindingLifecycleService` authorizes before any store access. A denial returns `Denied`
without observing finding existence. The store requires
`IndexDriftFindingAuthorizedLifecycleCommand`, whose constructor is private to the service, so an
ordinary command cannot bypass authorization through the public store trait.

## PostgreSQL transaction recheck

The PostgreSQL store uses one `SERIALIZABLE READ WRITE` transaction and performs:

1. command-UUID advisory locking;
2. exact replay-event comparison;
3. exact tenant/finding `FOR UPDATE` locking;
4. explicit open-state comparison;
5. state plus `closed_at` transition;
6. one actor/action/reason audit insert;
7. one atomic commit.

The audit insert and finding transition cannot commit separately. Missing findings and changed state
return typed `NotApplied` without audit insertion. A same-payload command retry returns
`AlreadyApplied`; command UUID reuse with a different payload fails permanently.

The finding key, check name, severity, scope, details, digest evidence, and detection timestamps are
not modified.

## Audit migration recheck

Migration `m20260806_000006_add_index_finding_lifecycle_audit`:

- is ordered after the locale-scope finding migration;
- rejects unsupported MySQL before partial DDL;
- creates an exact composite foreign key to `(tenant_id, finding_id)`;
- constrains action, from-state, target state, actor fields, and reason;
- uses database-generated event timestamps;
- prevents historical row updates in PostgreSQL and SQLite;
- preserves the established tenant/finding cascade retention path.

The lifecycle API exposes no audit update or delete operation. Retention deletion and historical row
rewriting are deliberately treated as different concerns.

## Disclosure and capability recheck

- command and actor `Debug` output omit actor subject and reason text;
- the authorized capability exposes only the validated command reference;
- the private stored replay-event type has no automatic `Debug` implementation;
- failures expose only bounded machine codes and retryable/permanent classification;
- receipts expose only command UUID, finding UUID, and resulting state;
- no runtime extension, GraphQL, HTTP, CLI, MCP, native-admin, scheduler, background loop, or repair
  capability is added.

## Deliberate non-claims

This recheck does not claim:

- Rust compilation or warning-free status;
- successful migration execution on PostgreSQL or SQLite;
- verifier, formatting, Cargo, workflow, or CI success;
- mounted authorization or command transport;
- audit inspection or retained production evidence;
- targeted or automatic repair.

The implementation agent did not run tests, JavaScript verifiers, formatting, Cargo commands,
migrations, database scenarios, workflows, or CI.

## Next cursor

Add one internal targeted repair boundary for an exact supported open finding. It must require a
non-public authorized operator capability, capture admitted before evidence, invoke one
finding-specific repair owner, capture admitted after evidence, and persist a separate idempotent
repair receipt. It must not infer repair authority from lifecycle state or add automatic iteration or
public transport.
