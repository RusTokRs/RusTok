# Operations Assets (`ops/`)

This directory contains operational and non-runtime assets that support development, performance engineering, and observability.

These folders are intentionally **not** application source code, but they are **not placeholders** and should not be treated as disposable.

## Structure

- `ops/benches/` — Rust benchmark crate (`rustok-benchmarks`) with Criterion scenarios used for performance regression checks and baseline measurements.
- `ops/grafana/` — Grafana provisioning assets (dashboards + datasources) used by `docker-compose.observability.yml`.
- `ops/prometheus/` — Prometheus scrape and alert rules consumed by `docker-compose.observability.yml`.

## Why it is in `ops/`

The repository root stays focused on product/runtime code, while operational tooling is grouped in one place for discoverability and maintenance.

## When you need these assets

- Daily feature work: usually optional.
- Performance work / profiling / SLO verification: required (`ops/benches`, `ops/grafana`, `ops/prometheus`).
- Local observability stack startup: required (`ops/grafana`, `ops/prometheus`).
