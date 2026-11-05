---
id: doc://docs/operations/README.md
kind: operations_documentation
language: en
source_language: en
status: current
canonical_for:
  - operations-overview
---

# Operations Documentation

This directory contains canonical operational specifications, runbooks, and runtime configuration references for operating the RusToK platform in development and production environments.

---

## Core Operations Documents

- [Platform Configuration & Environment Reference](./configuration-reference.md) — Comprehensive reference for all server configuration keys, YAML options, defaults, and environment variable overrides.
- [Tenant Runtime Profiles & Resolution Modes](./tenant-runtime-profiles.md) — Specification of multi-tenant, single-tenant, and development isolation profiles, header/subdomain resolution, and fail-closed security invariants.

---

## Operational Runbooks & Deployment Guides

- [Database Migrations & Safe Squashing Guide](../guides/safe-database-migrations-and-squashing.md) — Stepwise migration execution, transactional rollback, zero-downtime cutover, and schema parity verification.
- [Security Audit & Ingress Protection Guide](../guides/security-audit.md) — Verification runbook for SSRF/XSS sanitization, rate limiting, and RBAC security boundaries.
- [Architecture & Platform Overview](../architecture/overview.md) — Production topology, modular architecture, and distribution profiles.
