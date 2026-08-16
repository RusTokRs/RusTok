# End-to-end load tests

This directory owns reproducible HTTP/API load tests. It is intentionally separate from
`ops/benches`: Criterion and PostgreSQL evidence measure component/storage behavior,
while this suite measures externally observable request throughput and latency.

## RusTok vs Magento comparison

The canonical comparison contract is documented in
[`docs/benchmarks/rustok-vs-magento.md`](../../docs/benchmarks/rustok-vs-magento.md).

The first runnable slice is read-path only and covers:

- catalog list;
- product detail;
- catalog search;
- a deterministic mixed read workload.

Stateful cart/checkout workloads are deliberately not included in the first slice. They
must not be compared until both platforms use equivalent inventory, tax, shipping and
payment-stub semantics.

## Requirements

- k6 0.50 or newer;
- a dedicated load-generator host (do not run k6 on the system under test);
- one JSON adapter config per platform;
- identical benchmark data on both platforms;
- production/release builds with debug/profiling disabled unless the run explicitly
  declares a profiling profile.

## Run

```bash
cd ops/loadtest

k6 run \
  -e CONFIG=config/rustok.example.json \
  -e BASE_URL=http://127.0.0.1:5150 \
  -e OPERATION=mixed \
  -e RATE=500 \
  -e WARMUP=30s \
  -e DURATION=3m \
  -e PRODUCT_ID=<published-rustok-product-uuid> \
  -e PRODUCT_SKU=<shared-benchmark-sku> \
  -e SEARCH_TERM=shirt \
  k6/comparison.js
```

Run the same load profile against Magento by changing only `CONFIG`, `BASE_URL`, and the
target identity variables required by that adapter:

```bash
k6 run \
  -e CONFIG=config/magento.example.json \
  -e BASE_URL=http://127.0.0.1 \
  -e OPERATION=mixed \
  -e RATE=500 \
  -e WARMUP=30s \
  -e DURATION=3m \
  -e PRODUCT_SKU=<shared-benchmark-sku> \
  -e SEARCH_TERM=shirt \
  k6/comparison.js
```

`BASE_URL` overrides the example value stored in the adapter config, so production ingress
or local route prefixes can be selected without modifying tracked benchmark files.

The runner writes `summary.json`. Archive it together with system telemetry and the
run manifest described in the comparison contract.

## Supported environment variables

| Variable | Default | Meaning |
| --- | --- | --- |
| `CONFIG` | required | Adapter JSON file |
| `BASE_URL` | adapter value | Target scheme/host/port and optional route prefix |
| `OPERATION` | `mixed` | `catalog`, `product`, `search`, or `mixed` |
| `RATE` | `100` | Measured requests/second target |
| `WARMUP_RATE` | `RATE / 4` | Warm-up requests/second |
| `WARMUP` | `30s` | Warm-up duration |
| `DURATION` | `3m` | Measured duration |
| `PRE_ALLOCATED_VUS` | `64` | Initial VU pool |
| `MAX_VUS` | `2048` | Maximum VU pool |
| `PRODUCT_ID` | empty | RusTok product UUID placeholder |
| `PRODUCT_SKU` | empty | Shared SKU placeholder |
| `SEARCH_TERM` | `shirt` | Shared search term |
| `TENANT_ID` | empty | Optional tenant placeholder/header value |
| `CHANNEL` | empty | Optional channel placeholder/header value |

## Evidence rule

A throughput number is publishable only when the run also records:

1. exact RusTok commit and Magento release/build identifier;
2. CPU model, allocated vCPU, RAM, kernel/container limits and network topology;
3. PostgreSQL/MySQL, Redis/Valkey and search-engine versions and resource limits;
4. dataset cardinality and deterministic seed/hash;
5. cache profile (`app-cold`, `app-warm`, or `edge-cache`);
6. three successful repetitions with the same configuration;
7. p50/p95/p99, achieved RPS, error rate, CPU and peak/resident memory;
8. validation that returned business data is equivalent for the sampled fixtures.

Do not quote Criterion nanosecond timings, README throughput claims, or CDN/Varnish cache
hits as dynamic commerce RPS.