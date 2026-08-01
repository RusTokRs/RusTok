# FORUM-23B2G2B3D15 private and trusted-channel Search exclusion proof

## Status

`source_ready_maintainer_execution_pending`

This slice adds an executable PostgreSQL proof for the remaining exact private
and trusted-channel exclusion part of canonical `LINK-FORUM-03`. It does not run
PostgreSQL, Search, the storefront executor, or any verifier, and it does not
change the canonical link status from `planned`.

The machine contract is:

```text
crates/rustok-forum/contracts/forum-search-link-forum-03-private-trusted-exclusion-proof.json
```

The executable test is:

```text
apps/server/tests/forum_versioned_invalidation_private_trusted_exclusion.rs
```

The evidence path is:

```text
target/forum-search-link-forum-03-private-trusted-exclusion-evidence.json
```

## Why route channel alone is not privacy

Forum `channel_slugs` are an exact route-channel filter. A matching route may
still be public unless a richer owner audience layer requires authentication,
trust, channel membership, a group, a role, or an explicit user. D15 therefore
does not rename a route filter into a privacy guarantee.

The proof creates three topics through exported Forum owner services:

1. one unrestricted public control topic;
2. one private topic narrowed to one explicit user through the topic audience
   owner;
3. one trusted-channel topic whose visibility is the conjunction of:
   - exact route channel `trusted`;
   - an inherited category minimum trust level of 50;
   - a topic-local `channel_members_any = ["trusted"]` layer.

Because category and topic audience layers compose as a conjunction, the trusted
viewer must pass both the inherited trust layer and the topic membership layer.
The route-channel rule is evaluated independently before richer audience facts.

## Exact owner revision trace

The isolated tenant produces eight contiguous owner revisions:

| Revision | Owner mutation | Target |
| --- | --- | --- |
| 1 | public/private category create | `forum` |
| 2 | trusted category create | `forum` |
| 3 | unrestricted topic create | first category |
| 4 | private topic create | first category |
| 5 | trusted topic create | trusted category |
| 6 | explicit-user private topic policy | private topic |
| 7 | minimum-trust category policy | `forum` |
| 8 | trusted channel-membership topic policy | trusted topic |

The test reads the real root and caused typed envelopes from `sys_events`, checks
ledger, causation, payload and schema identity, and admits only the typed
envelopes through `ForumSearchContractIngress`. No Forum owner event or Search
inbox row is fabricated for this trace.

## Legitimate projection result

After all eight invalidations are admitted, the production
`ForumSearchProjectionSourceFactory` and `ForumProjectionReconciler` rebuild the
current owner state. The legitimate anonymous Search projection must contain
exactly:

- the unrestricted public category;
- the unrestricted public topic.

It must not contain:

- the explicit-user private topic;
- the trusted category;
- the trusted-channel topic.

This proves that canonical anonymous Search materialization does not downgrade
private or richer-audience content into public documents.

## Stale-candidate reauthorization matrix

After verifying the legitimate projection, the test deliberately inserts one
stale Search row for the private topic and one for the trusted-channel topic.
This is not a Forum mutation and does not stand in for owner behavior. It models
an already-stale Search candidate so the current owner reauthorization boundary
can be exercised through production storefront execution.

The expected matrix is:

| Candidate | Viewer | Expected |
| --- | --- | --- |
| private | public | denied |
| private | unrelated authenticated user | denied |
| private | exact explicitly allowed user | allowed |
| trusted | public on `trusted` route | denied |
| trusted | channel member below trust 50 | denied |
| trusted | trust 80 without membership | denied |
| trusted | trust 80 member on wrong route | denied |
| trusted | trust 80 member on `trusted` route | allowed |

Denied executions must expose zero items, zero total and no visible facet
buckets. Allowed executions must return exactly the intended topic identifier.

## Owner facts boundary

Authenticated trusted-channel checks use a test owner adapter implementing the
production `ForumAudienceFactsPort` contract. It returns only the exact requested
trust field or requested channel subset for the exact tenant and user. Every
request carries read deadline semantics.

The trusted storefront route context is internally complete and consistent: it
contains both a non-nil channel identifier and the matching slug, as required by
Search channel authority. D15 does not independently authenticate host Channel
resolution or replace the server adapter's module-admission and auth composition.

The test also records the request sequence:

- the low-trust member is resolved only for the inherited trust layer;
- the high-trust non-member resolves trust first and then the exact `trusted`
  membership candidate;
- the wrong-route viewer causes no facts request because route denial happens
  first;
- the exact trusted member resolves the same bounded trust and membership
  requests and is allowed.

Explicit-user private authorization remains local and does not call the optional
facts provider.

## Maintainer execution

On one exact checkout, run:

```bash
node scripts/verify/verify-forum-search-link-forum-03-private-trusted-exclusion-proof.mjs
RUSTOK_SEARCH_TEST_DATABASE_URL="$DATABASE_URL" \
  cargo test -p rustok-server \
  --test forum_versioned_invalidation_private_trusted_exclusion \
  -- --nocapture --test-threads=1
```

A passing test writes the evidence artifact only after every assertion succeeds
and after the isolated schema cleanup succeeds. A missing PostgreSQL URL skips
the test and creates no acceptable evidence.

## Deliberate boundary

D15 adds one executable test, one machine contract, one source verifier and this
handoff. It changes no production Rust path, migration, event schema or digest,
DTO, runtime flag, dependency, `Cargo.toml`, `Cargo.lock`, or canonical plan
status.

Topic move remains blocked on the planned `FORUM-21` owner command. D15 also does
not close arbitrary group, role, deny-list, attachment, topic-kind, or combined
policy permutations. A later bounded slice must assemble D13, D14 and D15 into a
complete reviewed `LINK-FORUM-03` artifact after the topic-move owner exists.

No command above was run by the implementation agent, per maintainer request.
