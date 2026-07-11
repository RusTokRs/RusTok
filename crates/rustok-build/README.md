# rustok-build

## Purpose

`rustok-build` owns platform build and release persistence contracts.

## Responsibilities

- Define build and release SeaORM models, status state machines, execution plans, and executor reports.
- Build and execute Cargo/Trunk command specifications independently of the server host.
- Execute queued build plans from server workers or `rustok-cli` through explicit event and release-activation ports.

## Interactions

`apps/server` composes workers, event delivery, and deployment adapters around these contracts. The future platform CLI will use the same capability without depending on the server application.

See [docs](docs/README.md).
