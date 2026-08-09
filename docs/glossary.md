---
id: doc://docs/glossary.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# RusTok Platform Glossary

Alphabetical reference guide for domain terminology, architectural patterns, and platform abstractions used throughout RusTok.

---

## A

### AgentPrincipal
An authorization identity assigned to an AI agent invocation (`rustok-ai`). Effective permissions during an AI run are calculated as the intersection of initiating user permissions and the agent principal's permissions ($\text{Subject} \cap \text{Agent}$), preventing privilege escalation.

### Alloy
The dynamic business logic and scripting extension layer ([`crates/alloy`](../crates/alloy/README.md)). Alloy runs sandboxed Rhai/WASM scripts (`rustok-sandbox`) for on-the-fly business rules, ETL data cleansing, and instant API/webhook integration without redeploying server binaries.

### Athanor
The native vector RAG (Retrieval-Augmented Generation) data plane and embedding engine inside `rustok-ai` (`rustok-ai-athanor`), providing semantic search and vector indexing directly attached to database engines.

---

## C

### CAS (Content-Addressable Storage)
The immutable storage paradigm used by `rustok-build-source` for module compilation materials. Build artifacts are identified and verified by cryptographic hashes rather than mutable file paths.

### Composition Root
The main host application (`apps/server`) responsible for loading `modules.toml`, assembling the `ModuleRegistry`, mounting Axum HTTP/GraphQL surfaces, and initializing background outbox workers.

### CursorCodec
The deterministic keyset cursor encoding mechanism in `rustok-index`. Converts composite primary keys and sorting values into checksummed base64 tokens for $O(1)$ keyset pagination without SQL `OFFSET` performance degradation.

---

## F

### FBA (Fluid Backend Architecture)
RusTok's backend architectural pattern that decouples domain traits from transport boundaries. Modules execute as zero-overhead in-process Rust trait calls in monolith mode, or as remote **gRPC** services when deployed in microservice topology, without rewriting business logic.

### FFA (Fluid Frontend Architecture)
RusTok's framework-agnostic UI architecture. UI state machines, view-models, input validation, and i18n catalogs are written in pure Rust (`rustok-ui-core`). Thin view-adapters allow swapping or upgrading rendering hosts (Leptos, Dioxus, Next.js, Flutter Mobile) without touching domain UI logic.

### Flex
The dynamic attribute and runtime entity extension module ([`crates/flex`](../crates/flex/README.md)). Allows adding typed custom properties to entities at runtime without performing database DDL schema migrations.

---

## I

### Instance Root
The operator-selected host-local directory that anchors one RusToK installation
on a standalone host or one node placement. Its canonical relative subtrees
hold configuration, operations tools, releases, sources, local object storage,
state, work, caches, logs, and runtime files. The path may be anywhere supported
by the operating system and is placement/restart evidence only; it never becomes
release, module, migration, object, or cross-node operation identity.

### Iggy (`outbox_iggy`)
Ultra-fast, native Rust event streaming broker ([https://iggy.rs](https://iggy.rs)) integrated via `rustok-iggy`. Provides append-only event logs, consumer groups, and durable Event Replay for high-throughput distributed deployments.

---

## M

### Matryoshka Composition
The nested module wrapper pattern where umbrella crates (such as `rustok-commerce` or `rustok-marketplace`) aggregate multiple focused sub-modules (`product`, `cart`, `order`, `ledger`, `payout`) while preserving individual module ownership and manifests.

### Model Context Protocol (MCP)
An open protocol for AI agent operations implemented in `rustok-mcp`. Allows external AI assistants (Claude, Cursor, custom agents) to safely inspect platform state, manage modules, and invoke tools.

### ModuleRegistry
The central runtime container in `apps/server` built during startup from `modules.toml`. Validates module manifests, manages lifecycle hooks (init, start, shutdown), and routes event listeners.

---

## P

### PortContext
The transport-agnostic context structure passed across every module service port (`crates/rustok-api/src/ports.rs`). Carries `tenant_id`, `actor`, OpenTelemetry trace identifiers (`correlation_id`, `causation_id`, `traceparent`), `idempotency_key`, and deadline propagation (`deadline_ms`).

---

## R

### Runtime Guardrails
The real-time operational health aggregation subsystem (`GET /health/runtime`). Monitors live signals: rate-limiter memory saturation, event bus backpressure depth, Iggy relay fallback state, and remote executor lease status.

---

## T

### Transactional Outbox (`sys_events`)
The event delivery pattern in `rustok-outbox`. Entity mutations and cross-module `DomainEvent` envelopes are written to the database table `sys_events` within the same atomic SQL transaction, guaranteeing zero event loss.

### Translation Memory (TM)
The cross-entity translation reuse repository in `rustok-translation`. Prevents redundant human or AI machine translation costs by matching identical source text strings across products, categories, and content pages.

### Turso (libSQL)
The serverless database engine supported alongside PostgreSQL. Provides per-tenant physical database isolation (`database-per-tenant`), zero-cost scale-to-zero, in-memory embedded replicas (< 1ms read latency), and 5ms instant database branching.
