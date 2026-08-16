# End-to-end load tests

This directory owns reproducible HTTP/API load tests. It is intentionally separate from
`ops/benches`: Criterion and PostgreSQL evidence measure component/storage behavior,
while this suite measures externally observable request throughput, latency and resource use.

The canonical RusTok-vs-Magento comparison contract is
[`docs/benchmarks/rustok-vs-magento.md`](../../docs/benchmarks/rustok-vs-magento.md).

The first runnable slice is read-path only:

- catalog list;
- product detail;
- catalog search with a shared title token of known cardinality;
- deterministic mixed reads (50% catalog / 35% product / 15% search).

The v1 Magento adapter is `magento-core-graphql`: it targets the core Magento Open Source / Adobe
Commerce `products` GraphQL contract. Adobe Commerce SaaS Catalog Service and Live Search expose
different product/search contracts and must use separate adapters/topologies rather than being
mixed into the core-GraphQL result.

Stateful cart/checkout workloads remain a separate phase because inventory reservation,
tax, shipping and payment-stub semantics must be equivalent before their timings are useful.

## Static verification

All load-test tooling is dependency-free Node.js except k6 itself.

```bash
node ops/loadtest/verify.mjs
```

The verifier checks JavaScript syntax and JSON contracts, generates the same 100-product fixture
twice to assert byte-identical JSONL/CSV, verifies fail-closed evidence creation, and on Linux
smoke-tests the process sampler plus result summarizer with synthetic k6 metrics.

## 1. Generate deterministic fixtures

```bash
node ops/loadtest/fixtures/generate.mjs \
  --tier m \
  --seed rustok-vs-magento-v1 \
  --out target/loadtest-fixtures/m
```

Tiers:

| Tier | Products | Variants/product |
| --- | ---: | ---: |
| `s` | 10,000 | 2 |
| `m` | 100,000 | 2 |
| `l` | 1,000,000 | 2 |

The generator streams its output and produces:

- `products.jsonl` — canonical platform-neutral fixture;
- `products.csv` — flat inspection/import interchange file;
- `manifest.json` — seed, counts, selected benchmark SKUs, search terms/cardinality and SHA-256 digests.

Every product title contains exactly one `bench-group-NN` search token. The manifest records
the exact expected match count for each token, so the search workload validates semantic parity
rather than only HTTP success.

## 2. Import the same fixture into RusTok

The importer uses the real commerce admin product contract: one localized translation, one
`Edition` option, two variants, deterministic prices, benchmark metadata and `publish: true`.
It writes a JSONL receipt mapping the shared fixture SKU to the created RusTok product ID.

```bash
export RUSTOK_ADMIN_PRODUCTS_URL='http://127.0.0.1:5150/admin/products'
export RUSTOK_TOKEN='<admin-token>'
export RUSTOK_TENANT_ID='<tenant-uuid>'
export RUSTOK_LOCALE='en'

node ops/loadtest/fixtures/import-rustok.mjs \
  --input target/loadtest-fixtures/m/products.jsonl \
  --receipt target/loadtest-fixtures/m/rustok-receipts.jsonl \
  --concurrency 16
```

`RUSTOK_ADMIN_PRODUCTS_URL` is intentionally the full route. Deployments may mount commerce
under an additional ingress/module prefix, and the fixture tool must not guess it.

Interrupted imports can continue with `--resume`; only receipt rows with `status: created`
are skipped. Failed rows stay visible in the receipt. Concurrency is bounded in product-sized
batches, so an import failure cannot disappear from a detached promise.

## 3. Import the same fixture into Magento

The Magento importer creates one visible configurable parent and two non-individually-visible
simple children, assigns the configured option, then links both children to the parent.
Installation-specific configurable attribute IDs are mandatory; the tool refuses to invent
them.

Provision a two-value configurable attribute (for example `bench_edition`) in the chosen
attribute set, then provide its exact IDs:

```bash
export MAGENTO_BASE_URL='http://127.0.0.1'
export MAGENTO_STORE_CODE='default'
export MAGENTO_TOKEN='<admin-token>'
export MAGENTO_ATTRIBUTE_SET_ID='4'
export MAGENTO_CONFIG_ATTRIBUTE_ID='<attribute-id>'
export MAGENTO_CONFIG_ATTRIBUTE_CODE='bench_edition'
export MAGENTO_CONFIG_ATTRIBUTE_LABEL='Edition'
export MAGENTO_CONFIG_VALUE_1='<default-value-index>'
export MAGENTO_CONFIG_VALUE_2='<alternate-value-index>'

node ops/loadtest/fixtures/import-magento.mjs \
  --input target/loadtest-fixtures/m/products.jsonl \
  --receipt target/loadtest-fixtures/m/magento-receipts.jsonl \
  --concurrency 4
```

This synchronous importer favors auditability and resumability over fixture-import speed.
Import duration is not a benchmark metric. A future bulk importer may replace it without
changing the canonical fixture manifest.

## 4. Create an immutable evidence run

Copy the topology example outside the tracked example and replace every placeholder with the
actual process/container/VM limits and dependency versions.

```bash
cp ops/loadtest/evidence/topology.example.json target/loadtest-topology.json
# edit target/loadtest-topology.json with exact benchmark resources/versions

RATE=500 \
WARMUP=30s \
DURATION=3m \
OPERATION=mixed \
PRODUCT_SKU=RTBM-00050001 \
SEARCH_TERM=bench-group-00 \
SEARCH_EXPECTED_MATCHES=5000 \
node ops/loadtest/evidence/create-run.mjs \
  --fixtures target/loadtest-fixtures/m/manifest.json \
  --topology target/loadtest-topology.json \
  --run-id m-p1-r4-500rps \
  --rustok-commit "$(git rev-parse HEAD)" \
  --magento-release '<exact-release>' \
  --profile P1 \
  --workload R4
```

Use the exact `expected_matches` value from the selected manifest search case; `5000` is the
expected value for each of 20 groups only in the canonical 100k-product tier.

The writer creates `evidence/rustok-vs-magento/<run-id>/` with `rustok/`, `magento/` and a
non-overwritable `manifest.json`. It re-hashes `products.jsonl` and `products.csv`, pins the
topology hash, validates the selected search term/count against the fixture manifest, and refuses
unresolved `REPLACE_ME`/`unknown`/`unresolved` values by default. `--allow-placeholders` is for
harness development only and must not be used for publishable evidence.

Use one run directory per dataset/profile/workload/rate point. The three repetitions for that
point live inside the same directory.

## 5. Collect measured-window resources

On a Linux system-under-test host, sample the processes that belong to the compared stack.
Use the canonical target name `app` for the application process because the summarizer uses it
by default.

For a 30-second warm-up plus 3-minute measured run:

```bash
node ops/loadtest/evidence/collect-process.mjs \
  --target app:<pid> \
  --target db:<pid> \
  --target cache:<pid> \
  --target search:<pid> \
  --delay-ms 30000 \
  --duration-ms 180000 \
  --interval-ms 1000 \
  --output evidence/rustok-vs-magento/m-p1-r4-500rps/rustok/telemetry-run-1.jsonl
```

The collector records RSS, high-water RSS, thread counts and user/system CPU ticks. It records
`CLK_TCK` and page size in the telemetry metadata rather than assuming Linux defaults.

If components live on different hosts, run one collector on each host. The current
`summarize.mjs` `sampled_stack_*` totals refer only to processes present in one telemetry file;
do not call those values whole-stack totals until host-level evidence has been combined.

## 6. Run k6

From `ops/loadtest`:

```bash
k6 run \
  -e CONFIG=config/rustok.example.json \
  -e BASE_URL=http://127.0.0.1:5150 \
  -e OPERATION=mixed \
  -e RATE=500 \
  -e WARMUP=30s \
  -e DURATION=3m \
  -e PRODUCT_ID=<rustok-id-from-receipt> \
  -e PRODUCT_SKU=RTBM-00050001 \
  -e SEARCH_TERM=bench-group-00 \
  -e SEARCH_EXPECTED_MATCHES=5000 \
  k6/comparison.js
```

Run exactly the same arrival-rate profile for Magento by changing only its adapter/base URL and
platform-specific product identity where required.

The search adapter on both platforms checks the shared token **and** exact total hit count from
`SEARCH_EXPECTED_MATCHES`. A `200 OK` with different business results therefore fails validation.

The runner separates warm-up from measurement, counts `measured_requests` independently and
fails the measured scenario if k6 drops iterations. It also enforces the read SLO:

- p95 < 250 ms;
- p99 < 500 ms;
- HTTP failure rate < 0.1%;
- response-validation failure rate < 0.1%;
- zero dropped measured iterations.

Move each generated `summary.json` into its immutable run directory as
`summary-run-1.json`, `summary-run-2.json`, etc.

## 7. Summarize one rate-ladder step

```bash
node ops/loadtest/evidence/summarize.mjs \
  --summary evidence/rustok-vs-magento/m-p1-r4-500rps/rustok/summary-run-1.json \
  --telemetry evidence/rustok-vs-magento/m-p1-r4-500rps/rustok/telemetry-run-1.jsonl \
  --app-target app \
  --application-vcpu 4 \
  --output evidence/rustok-vs-magento/m-p1-r4-500rps/rustok/result-run-1.json
```

The summarizer refuses old/ambiguous summaries without the measured-only `measured_requests`
metric and requires at least two valid `app` telemetry samples. The result contains achieved
measured RPS, p50/p95/p99, failures, dropped iterations, SLO pass, application average CPU cores,
peak RSS/HWM, sampled-stack CPU/RSS and normalized `app_rps_per_vcpu` / `app_mib_per_1k_rps`.

## k6 environment variables

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
| `PRODUCT_SKU` | empty | Shared parent SKU placeholder |
| `SEARCH_TERM` | `shirt` | Shared title search token; use a manifest case for evidence |
| `SEARCH_EXPECTED_MATCHES` | empty | Exact `expected_matches` for `SEARCH_TERM`; required by evidence search/mixed runs |
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
8. exact search cardinality and sampled business-data parity.

Do not quote Criterion nanosecond timings, README throughput claims, or CDN/Varnish cache hits
as dynamic commerce RPS.
