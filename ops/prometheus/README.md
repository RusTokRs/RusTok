# Prometheus Config (`ops/prometheus`)

This directory contains Prometheus configuration used by the local observability stack.

## What is here

- `prometheus.yml` — scrape/evaluation configuration.
- `alert_rules.yml` — alerting rules for SLO/error/latency style checks.

## How it is used

`docker-compose.observability.yml` mounts these files into the Prometheus container and starts Prometheus with `--config.file=/etc/prometheus/prometheus.yml`.

This is active runtime-observability configuration, not a placeholder.

## JWT operational signals

See the [Auth owner contract](../../crates/modules/rustok-auth/docs/README.md#operational-observability).
Scrape each host with deployment host-read authority, not a single load-balanced
target. Monitor up and expected target inventory separately. Validate the rules
with `promtool check rules ops/prometheus/alert_rules.yml` before deployment.
Thresholds are initial operational defaults, not lease/admission policy.
