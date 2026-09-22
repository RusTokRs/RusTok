# Grafana Provisioning (`ops/grafana`)

This directory contains Grafana provisioning files used by the local observability stack.

## What is here

- `dashboards/`
  - `dashboard.yml` — dashboard provisioning config.
  - `rustok-overview.json` — overview dashboard.
  - `rustok-advanced.json` — advanced dashboard.
- `datasources/`
  - `datasources.yml` — datasource provisioning (Prometheus target).

## How it is used

`docker-compose.observability.yml` mounts this directory into Grafana provisioning paths so dashboards/datasources appear automatically on startup.

This directory is actively used operational config, not placeholder content.
