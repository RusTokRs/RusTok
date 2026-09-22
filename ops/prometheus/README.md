# Prometheus Config (`ops/prometheus`)

This directory contains Prometheus configuration used by the local observability stack.

## What is here

- `prometheus.yml` — scrape/evaluation configuration.
- `alert_rules.yml` — alerting rules for SLO/error/latency style checks.

## How it is used

`docker-compose.observability.yml` mounts these files into the Prometheus container and starts Prometheus with `--config.file=/etc/prometheus/prometheus.yml`.

This is active runtime-observability configuration, not a placeholder.
