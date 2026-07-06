---
id: doc://docs/research/medusa-vs-rustok-architecture.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# Architecture Comparison: RusTok vs Medusa

## Why This Document Exists

This document captures an architectural comparison between RusTok and Medusa in the context of the
`Medusa JS clone` goal for the ecommerce family. It addresses three practical questions:

1. how technically close Medusa is to a modular monolith compared to classic microservices;
2. how compatible the current RusTok architecture is with the Medusa-style approach;
3. where achieving parity is realistic and where direct reuse cannot be expected.

Medusa's state in this comparison was verified against official Medusa documentation as of
`2026-04-08`.

## Short Conclusion

Medusa is technically closer to a modular monolith with pluggable infrastructure and
workflow-orchestration than to classic microservices architecture.

RusTok in its current form is architecturally closer to Medusa than to a service-per-domain
landscape:

- both systems are built around a single application runtime;
- both separate domains into isolated modules;
- both prefer composition through a container/module registry rather than network hops between
  internal domains;
- both keep third-party integrations on provider/module seams.

However, RusTok and Medusa are not identical:

- Medusa is more tightly coupled to JS/TS container + workflow engine + plugin/module/provider
  contracts;
- RusTok is more heavily based on Rust crate boundaries, manifest-driven composition and a thin-host
  model;
- Medusa designs commerce as a set of modules within a single application;
- RusTok designs a platform broader than ecommerce and uses commerce as one of the module
  families.

## What Makes Medusa Closer to a Modular Monolith

Per the official architecture, a Medusa application looks like a single runtime where:

- HTTP/API routes trigger workflows;
- workflows coordinate commerce and infrastructure modules;
- modules register in the application container;
- modules receive a connection to a single configured PostgreSQL database;
- integrations are connected through module/provider seams, not as mandatory
  internal microservices.

This is visible in official documents:

- [Medusa Architecture](https://docs.medusajs.com/learn/introduction/architecture)
- [Modules](https://docs.medusajs.com/learn/fundamentals/modules)
- [Commerce Modules](https://docs.medusajs.com/learn/fundamentals/modules/commerce-modules)
- [Core Workflows Reference](https://docs.medusajs.com/resources/medusa-workflows-reference)
- [Plugins](https://docs.medusajs.com/learn/fundamentals/plugins)

From this, important architectural characteristics follow:

1. `single runtime`
Medusa does not require a separate process per domain by default.

2. `shared container`
Module services are resolved from a shared application container.

3. `shared database baseline`
Modules live in a single application-level database environment, not in separate service
datastores by default.

4. `internal orchestration instead of network choreography`
Relations between domains typically go through workflows/steps inside the process, not through
HTTP/gRPC between internal services.

5. `pluggable infrastructure instead of distributed core`
Redis, file storage, analytics, locking and external commerce providers are connected as
swappable providers/modules, but the core application does not become a set of
independent business services.

Therefore, Medusa's marketing narrative of "composable" and "service integrations" is
technically not equivalent to classic microservices architecture.

## Where RusTok and Medusa Are Actually Similar

### 1. Both projects are modular, not service-per-domain

In Medusa, modules are the basic unit of business capability. In RusTok, the basic unit
is also a module/crate family.

Practically, this means:

- domain boundaries are expressed by code and contracts;
- extension goes through module seams;
- internal domains are not required to be separate deployment units.

### 2. Both projects keep orchestration separate from domain services

In Medusa, routes typically call workflows, and workflows use module services.

In RusTok, host and transport routes call application/domain services in crates, while
`apps/server` remains a thin host.

### 3. Both projects prefer extensibility through providers/adapters

Medusa uses module providers for fulfillment, auth, file, locking, analytics and
other capabilities.

RusTok moves in the same direction through provider SPI, module boundaries and integration seams.

### 4. Both projects allow platform-level domain reuse

In Medusa, core commerce logic is available not only through API but also directly from custom flows.

In RusTok, module services are also intended as reusable building blocks for transport,
UI and orchestration layers.

### 5. Both projects support marketplace/composability narrative without mandatory microservices

Both Medusa and RusTok can be "composable" without transitioning to a distributed system inside
the core.

## Where RusTok and Medusa Fundamentally Differ

### 1. Medusa module model is tightly coupled to JS/TS runtime

Medusa expects:

- JS/TS modules;
- container registration names;
- workflow steps/workflow SDK;
- plugin packaging via npm;
- `medusa-config.ts` as composition point.

RusTok uses:

- Rust crates;
- manifest-driven module wiring;
- host-side composition through platform registry;
- its own transport/runtime contracts.

This is the main technical obstacle to direct reuse.

### 2. Medusa workflow engine is a central part of app semantics

In Medusa, the workflow layer participates in core flow semantics, rollback and extension hooks.

In RusTok, orchestration is expressed through application services, state machines, event flow and
module-owned transport contracts, not through a Medusa workflow SDK.

### 3. Medusa modules are designed for Medusa container contracts

Medusa customizations are built around the ability to resolve a module service from the
container and use it in workflows/routes/subscribers/jobs.

RusTok services were not designed as Medusa container resources and do not implement their
interfaces.

### 4. Medusa plugins are broader than module semantics

A Medusa plugin can simultaneously contain:

- modules;
- workflows;
- API routes;
- subscribers;
- scheduled jobs;
- admin extensions.

In RusTok, this is organized differently: module crate, host composition, UI package, docs, manifest.

### 5. Medusa thinks commerce-first, RusTok thinks platform-first

Medusa is architecturally centered around ecommerce.

RusTok is centered around a broader platform/module system, where ecommerce is only one
of the large bounded families.

### 6. Medusa by default more strongly unifies lifecycle around its own domain model

If using Medusa as the primary runtime, its workflows and modules expect certain
input/output semantics, provider ids, actor model, lifecycle transitions and data ownership.

RusTok can implement similar logic, but this does not imply automatic binary/runtime
compatibility.

## 10 Similarities

1. Both projects are architecturally closer to a modular monolith than to classic microservices.
2. Both projects use modules as the primary domain boundary.
3. Both projects support third-party integrations through provider/module seams.
4. Both projects keep orchestration above domain services.
5. Both projects allow reuse of core domain logic outside a pure HTTP layer.
6. Both projects require explicit contract boundaries between transport and domain.
7. Both projects allow gradual expansion of commerce through bounded contexts.
8. Both projects can live in a single database/runtime baseline without losing composability.
9. Both projects require parity discipline to prevent transport from detaching from the domain model.
10. Both projects benefit from a thin host/composition root instead of business logic in routing layer.

## 10 Differences

1. Medusa is implemented as a JS/TS application platform, RusTok as a Rust module platform.
2. Medusa container and workflow SDK are a mandatory part of the extension model.
3. RusTok uses manifest-driven composition, Medusa uses plugin/module registration.
4. Medusa standardizes provider interfaces deeper around its own framework.
5. RusTok more strongly separates the platform host from module crates and publishable UI packages.
6. Medusa out of the box standardizes more ecommerce actor/provider semantics.
7. RusTok has a broader platform scope and is not limited to commerce.
8. Medusa extension ecosystem is oriented toward npm packages, RusTok toward crate/module workspace.
9. Medusa workflows are the canonical orchestration seam; in RusTok this is not a central
   runtime primitive.
10. Direct in-process reuse between the systems is nearly absent due to runtime model mismatch.

## What This Means for the `Medusa JS Clone` Goal

The goal of "making a Medusa JS clone" for RusTok is technically realistic if understood as:

- replicate bounded contexts;
- replicate domain semantics;
- replicate transport surface and operator flows;
- replicate provider seams;
- replicate lifecycle expectations;
- but not attempt to replicate Medusa's internal JS runtime one-to-one.

That is, parity should be:

- `semantic parity`;
- `API/flow parity`;
- `operator capability parity`;
- `domain lifecycle parity`.

Not required:

- `runtime implementation parity`;
- `plugin binary compatibility`;
- `in-process module compatibility`.

## Can We Use Our Modules Inside Medusa

### Short Answer

Directly "as-is" — hardly.

Through an adapter/plugin/provider layer — yes, partially realistic.

### What Prevents Direct Reuse

- our modules are not written for Medusa container contracts;
- they do not implement Medusa module/provider interfaces;
- they are not integrated into the Medusa workflow SDK;
- they are not packaged as Medusa plugins/modules for `medusa-config.ts`;
- their domain ownership and transport contracts were designed for RusTok host.

### Where Integration Is Realistic

Most realistic scenarios:

1. `provider-style integration`
Connect RusTok capability as an external backend behind a Medusa provider/module adapter.

2. `service-backed custom module`
Write a Medusa custom module that calls RusTok API and uses RusTok as a system of
record for part of the capability.

3. `headless sidecar integration`
Use Medusa as a storefront/admin/runtime ecosystem, and RusTok as a separate
headless commerce backend for some domains.

### Where Integration Will Be Most Expensive

Most expensive areas:

- cart/order/checkout as core flows;
- inventory reservation semantics;
- pricing/promotions;
- post-order changes/returns/refunds;
- any workflow-heavy lifecycle paths.

The reason is simple: in those areas, Medusa relies not only on similar entities but also on its own
orchestration, compensation/rollback semantics and data ownership expectations.

## Recommended Position for RusTok

For RusTok, it is useful to look at Medusa as:

- a good reference architecture for ecommerce domains;
- a good reference transport surface for `/store/*` and `/admin/*`;
- a good reference operator/provider seam model;
- but not as a runtime with which to achieve direct module compatibility.

Practical recommendations:

1. build Medusa-compatible semantics and API shape where it brings value;
2. do not design RusTok crates for in-process reuse inside Medusa;
3. if integration with Medusa is ever needed, implement it through an adapter/plugin/provider
   layer rather than attempting to "insert" RusTok modules into the Medusa runtime.

## Sources

- [Medusa Architecture](https://docs.medusajs.com/learn/introduction/architecture)
- [Medusa Modules](https://docs.medusajs.com/learn/fundamentals/modules)
- [Medusa Commerce Modules](https://docs.medusajs.com/learn/fundamentals/modules/commerce-modules)
- [Medusa Plugins](https://docs.medusajs.com/learn/fundamentals/plugins)
- [Medusa Config / modules/plugins registration](https://docs.medusajs.com/learn/configurations/medusa-config)
- [Medusa Core Workflows Reference](https://docs.medusajs.com/resources/medusa-workflows-reference)
- [Fulfillment Module Provider](https://docs.medusajs.com/resources/commerce-modules/fulfillment/fulfillment-provider)
- [Infrastructure Modules](https://docs.medusajs.com/resources/infrastructure-modules)
