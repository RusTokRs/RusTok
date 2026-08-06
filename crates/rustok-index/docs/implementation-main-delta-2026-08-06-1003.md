# Index implementation main delta — 2026-08-06 10:03 UTC

Latest checked default branch: `main@26ee12603058324a71b49d2064e95cfd9214d007`.

The confirmed-candidate persistence branch started from
`main@b18558e3f8443135a403780d1ef75bc4d14dba6d`.

## Intervening default-branch changes

The intervening commits:

- bound Commerce GraphQL Cart/Pricing port diagnostics;
- bound Commerce legacy GraphQL helper diagnostics;
- exposed Forum owner-authorized deleted topic routes through additive storefront decisions and
  private no-store HTTP 410 composition.

The Commerce commits modify Commerce GraphQL diagnostic projection, Commerce documentation,
evidence, and Commerce-specific verifiers. The Forum commit modifies Forum/storefront route DTOs,
GraphQL/native transport, host HTTP composition, Forum contracts, documentation, and Forum-specific
verifiers.

They do not modify:

- `crates/rustok-index/src/infrastructure/postgres/drift_confirmed_candidate_writer.rs`;
- Index finding storage, candidate confirmation, reader, or application contracts;
- Index crate exports changed by this branch;
- Index persistence documentation or static guards;
- Index server services, GraphQL transports, lifecycle commands, scheduler, or repair code.

## Merge conclusion

The branch remains a separate internal Index slice. Its changed files are limited to the confirmed
candidate PostgreSQL writer, crate exports, architecture documentation, source recheck, plan, and
static guards. No semantic or textual overlap requiring branch changes was found.

No tests, verifiers, formatting, Cargo checks, PostgreSQL scenarios, workflows, or CI were executed by
the implementation agent.
