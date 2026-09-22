# Profiles checkpoint: moving-window external observer evidence

Status: **source-complete retained tooling; canonical packet is pending**.

## Profiles boundary

Profiles never authorizes from moving-window applicability, broker state, private cursors, duplicate counts, reset review, retained evidence, or observer availability. This harness belongs to Iggy/event-delivery operations only.

The source-complete fixture checks a production-reachable cross-cycle case in the same production-selected partition:

```text
cycle 1: first physical copy
cycle 2: second identical physical copy
cycle 3: no new message, retained duplicate remains visible
replacement observer: reset to initial_offset = 0
```

No partition identity, offset, deterministic message ID, payload, digest, credential, or raw error crosses into Profiles.

## Reviewed reset boundary

The capture requires an external review with:

```text
initial_offset = 0
restart_continuity_required = false
acceptable_reset_frequency = reviewed bounded label
review_scope = reviewed bounded label
```

This does not approve restart-safe progress. If restart continuity is required, a separately owned persistent cursor design must be reviewed before evidence can claim it.

## Retained packet boundary

The no-clobber packet may retain only canonical reviewed projections, bounded artifact/toolchain labels, source hashes, timestamps, count-only assertions, and test-output digest/size. It must not retain endpoints, file paths, credentials, broker coordinates, message identities, payloads, or raw output.

The exact runtime test and capture are source-complete, but the canonical packet is pending because no external Iggy execution was run by the implementation agent.

## Remaining work

1. run the locked case on a reviewed disposable external Iggy deployment;
2. inspect and commit the no-clobber packet;
3. retain full server-process execution separately;
4. add persistent cursor ownership only if restart continuity is required;
5. repeat separately for bundled, TLS/auth/failover, and multi-replica operations.

Tests, Cargo commands, source verifiers, broker execution, server startup, and retained capture were not run by the implementation agent.
