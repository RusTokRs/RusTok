# Index storage evidence comparison

Generated: 2026-07-26T23:05:43.874Z

> Evidence summary only. The first repetition is a first-run signal and later repetitions form the warm median; this is not a guaranteed OS cold-cache test.

Decision ready: **yes**

## Decision contract

- Required 100k/1m scales: **yes**
- Same packet contract version: **yes**
- Same result digest contract: **yes**
- Same repository: **yes**
- Same commit: **yes**
- Same PostgreSQL image/settings: **yes**
- Compared PostgreSQL fields: `server_version_num`, `shared_buffers`, `effective_cache_size`, `work_mem`, `random_page_cost`, `jit`, `standard_conforming_strings`, `timezone`, `date_style`, `extra_float_digits`
- Same repetitions/churn contract: **yes**
- Same non-scale dataset shape: **yes**
- Same source-oracle shape: **yes**
- Same candidate/workload shape: **yes**
- Same mutation effect contract: **yes**

## 100k evidence

- Packet contract: `v2`
- Result digest contract: `ordered_length_prefixed_json_v1`
- Repository: `RusTokRs/RusTok`
- Commit: `eae5f74241e9431bffe2fd8c43cd046fc1c1f679`
- Workflow run: `30222913450`
- PostgreSQL image: `postgres:16`
- Source load: 5014 ms

### Source oracle

| Workload | Result rows | Digest |
| --- | ---: | --- |
| status_equality | 100 | `4ad6336e71b37400a6765e30d56ebd29` |
| price_range_sort | 100 | `e13c87858433010e8262191f0f0513b9` |
| multi_value_tag | 100 | `ca1d7a330e7b136afb21d69ed4ebeb3b` |
| two_hop_channel_filter | 100 | `9480db66a1be8b6e2f768a7c56275d44` |
| keyset_page | 61 | `e989786d5952535720b16f97ce5e4f7c` |
| exact_count | 1 | `897f9cf1ebbe1918c58534b53e587ea5` |

| Prototype | Load | Schema size | Fields after churn | Churn growth | Dead tuples after churn | VACUUM |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| jsonb | 15612 ms | 449.72 MiB | n/a | 7.08 MiB (1.57%) | 20,000 | 943 ms |
| typed_eav | 42349 ms | 832.13 MiB | 1,400,160 | 10.73 MiB (1.29%) | 69,903 | 804 ms |
| hot_projection | 11479 ms | 361.54 MiB | n/a | 4.97 MiB (1.37%) | 20,000 | 672 ms |

### Read/query

| Prototype | Workload | First run | Warm median | First read blocks | Warm read blocks | Plan shapes |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| jsonb | status_equality | 2.58 ms | 2.25 ms | 0 | 0 | 1 |
| jsonb | price_range_sort | 1.78 ms | 1.63 ms | 0 | 0 | 1 |
| jsonb | multi_value_tag | 1.58 ms | 1.54 ms | 0 | 0 | 1 |
| jsonb | two_hop_channel_filter | 31.80 ms | 32.02 ms | 0 | 0 | 1 |
| jsonb | keyset_page | 0.10 ms | 0.06 ms | 0 | 0 | 1 |
| jsonb | exact_count | 2.26 ms | 1.94 ms | 0 | 0 | 1 |
| typed_eav | status_equality | 0.51 ms | 0.45 ms | 0 | 0 | 1 |
| typed_eav | price_range_sort | 0.58 ms | 0.48 ms | 0 | 0 | 1 |
| typed_eav | multi_value_tag | 0.57 ms | 0.48 ms | 0 | 0 | 1 |
| typed_eav | two_hop_channel_filter | 32.19 ms | 31.81 ms | 0 | 0 | 1 |
| typed_eav | keyset_page | 18.16 ms | 18.54 ms | 0 | 0 | 1 |
| typed_eav | exact_count | 11.99 ms | 11.46 ms | 0 | 0 | 1 |
| hot_projection | status_equality | 0.14 ms | 0.07 ms | 0 | 0 | 1 |
| hot_projection | price_range_sort | 0.11 ms | 0.10 ms | 0 | 0 | 1 |
| hot_projection | multi_value_tag | 1.38 ms | 1.30 ms | 0 | 0 | 1 |
| hot_projection | two_hop_channel_filter | 27.25 ms | 26.65 ms | 0 | 0 | 1 |
| hot_projection | keyset_page | 0.10 ms | 0.08 ms | 0 | 0 | 1 |
| hot_projection | exact_count | 2.24 ms | 1.86 ms | 0 | 0 | 1 |

### Mutation/WAL

| Prototype | Workload | Entities | Fields | Links | Median execution | Median WAL bytes (max node) | Peak WAL bytes (max node) | Plan shapes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| jsonb | update_product_batch | 1,000 | n/a | n/a | 42.59 ms | 1,066,436 | 1,125,595 | 1 |
| jsonb | delete_product_batch | 1,000 | n/a | 2,000 | 21.69 ms | 162,000 | 162,000 | 1 |
| typed_eav | update_product_batch | 1,000 | 2,000 | n/a | 74.22 ms | 1,318,162 | 1,336,010 | 1 |
| typed_eav | delete_product_batch | 1,000 | 8,000 | 2,000 | 55.41 ms | 594,000 | 594,000 | 1 |
| hot_projection | update_product_batch | 1,000 | n/a | n/a | 36.48 ms | 834,784 | 844,686 | 1 |
| hot_projection | delete_product_batch | 1,000 | n/a | 2,000 | 19.52 ms | 162,000 | 162,000 | 1 |

## 1m evidence

- Packet contract: `v2`
- Result digest contract: `ordered_length_prefixed_json_v1`
- Repository: `RusTokRs/RusTok`
- Commit: `eae5f74241e9431bffe2fd8c43cd046fc1c1f679`
- Workflow run: `30222913450`
- PostgreSQL image: `postgres:16`
- Source load: 39857 ms

### Source oracle

| Workload | Result rows | Digest |
| --- | ---: | --- |
| status_equality | 100 | `2d0c9482a04578526dbbce0bdaff8297` |
| price_range_sort | 100 | `195bb590eb6dabf314e299e01a610ce5` |
| multi_value_tag | 100 | `20c363844f408d2b4541d710c2f65c7a` |
| two_hop_channel_filter | 100 | `28ef39381985a678283f95f09e34e16f` |
| keyset_page | 100 | `a50ab417583d2e9606a68a1eb6582ddf` |
| exact_count | 1 | `9d753e3079b79503643c296531167bcb` |

| Prototype | Load | Schema size | Fields after churn | Churn growth | Dead tuples after churn | VACUUM |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| jsonb | 127965 ms | 4.25 GiB | n/a | 8.39 MiB (0.19%) | 21,916 | 3124 ms |
| typed_eav | 427256 ms | 8.14 GiB | 14,000,320 | 14.24 MiB (0.17%) | 69,270 | 16490 ms |
| hot_projection | 100939 ms | 3.53 GiB | n/a | 6.18 MiB (0.17%) | 20,496 | 2043 ms |

### Read/query

| Prototype | Workload | First run | Warm median | First read blocks | Warm read blocks | Plan shapes |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| jsonb | status_equality | 0.07 ms | 0.06 ms | 0 | 0 | 1 |
| jsonb | price_range_sort | 0.09 ms | 0.08 ms | 0 | 0 | 1 |
| jsonb | multi_value_tag | 12.31 ms | 11.61 ms | 0 | 0 | 1 |
| jsonb | two_hop_channel_filter | 258.95 ms | 255.55 ms | 29,115 | 29,116 | 1 |
| jsonb | keyset_page | 0.10 ms | 0.08 ms | 0 | 0 | 1 |
| jsonb | exact_count | 19.88 ms | 10.68 ms | 3,330 | 0 | 1 |
| typed_eav | status_equality | 0.47 ms | 0.48 ms | 0 | 0 | 1 |
| typed_eav | price_range_sort | 0.50 ms | 0.53 ms | 0 | 0 | 1 |
| typed_eav | multi_value_tag | 0.56 ms | 0.53 ms | 0 | 0 | 1 |
| typed_eav | two_hop_channel_filter | 246.73 ms | 251.46 ms | 25,133 | 25,837 | 1 |
| typed_eav | keyset_page | 156.22 ms | 152.45 ms | 36,454 | 36,445 | 1 |
| typed_eav | exact_count | 111.52 ms | 100.72 ms | 15,020 | 16,601 | 1 |
| hot_projection | status_equality | 0.09 ms | 0.08 ms | 0 | 0 | 1 |
| hot_projection | price_range_sort | 0.11 ms | 0.07 ms | 0 | 0 | 1 |
| hot_projection | multi_value_tag | 0.98 ms | 0.59 ms | 0 | 0 | 1 |
| hot_projection | two_hop_channel_filter | 236.50 ms | 231.83 ms | 29,113 | 29,115 | 1 |
| hot_projection | keyset_page | 0.07 ms | 0.08 ms | 0 | 0 | 1 |
| hot_projection | exact_count | 18.92 ms | 10.14 ms | 2,828 | 0 | 1 |

### Mutation/WAL

| Prototype | Workload | Entities | Fields | Links | Median execution | Median WAL bytes (max node) | Peak WAL bytes (max node) | Plan shapes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| jsonb | update_product_batch | 1,000 | n/a | n/a | 149.59 ms | 1,029,145 | 1,046,469 | 1 |
| jsonb | delete_product_batch | 1,000 | n/a | 2,000 | 132.95 ms | 162,000 | 162,000 | 1 |
| typed_eav | update_product_batch | 1,000 | 2,000 | n/a | 188.79 ms | 1,346,681 | 1,352,252 | 1 |
| typed_eav | delete_product_batch | 1,000 | 8,000 | 2,000 | 169.76 ms | 594,000 | 594,000 | 1 |
| hot_projection | update_product_batch | 1,000 | n/a | n/a | 142.32 ms | 810,672 | 837,139 | 1 |
| hot_projection | delete_product_batch | 1,000 | n/a | 2,000 | 133.10 ms | 162,000 | 162,000 | 1 |

## 1m / 100k ratios

### Source oracle result rows

| Workload | Result rows |
| --- | ---: |
| status_equality | 1.00x |
| price_range_sort | 1.00x |
| multi_value_tag | 1.00x |
| two_hop_channel_filter | 1.00x |
| keyset_page | 1.64x |
| exact_count | 1.00x |

### Storage candidates

| Prototype | Load | Schema | Field rows | VACUUM |
| --- | ---: | ---: | ---: | ---: |
| jsonb | 8.20x | 9.67x | n/ax | 3.31x |
| typed_eav | 10.09x | 10.01x | 10.00x | 20.51x |
| hot_projection | 8.79x | 9.99x | n/ax | 3.04x |

## Manual ADR inputs still required

- operational complexity and schema-evolution cost;
- index-management and migration strategy;
- acceptable latency, relation-size, WAL and maintenance trade-offs;
- selected model and explicit rejection rationale for the alternatives.

