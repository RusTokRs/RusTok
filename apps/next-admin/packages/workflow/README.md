# @rustok/workflow-admin

Module-owned Next.js administration package for the RusToK Workflows platform.

## Overview

This package provides the administration interface for event-driven workflows, visual step orchestration, execution tracing, and version control:

- **Workflows Management**: List, filter, inspect, and toggle workflow lifecycle states (`DRAFT`, `ACTIVE`, `PAUSED`, `ARCHIVED`).
- **Step Orchestration**: Configure linear and branching execution pipelines (`ACTION`, `CONDITION`, `DELAY`, `ALLOY_SCRIPT`, `EMIT_EVENT`, `HTTP`, `NOTIFY`, `TRANSFORM`).
- **Execution History**: Inspect real-time and historical workflow runs, step-by-step inputs/outputs, error logs, and execution duration.
- **Templates & AI Generation**: Initialize workflows from pre-built starter templates or generate definitions from natural language prompts.
- **Version Control & Rollbacks**: View immutable revision history and restore previous versions.

## Architecture

- **Host Integration**: Registers `/dashboard/workflows` under the `modulePlugins` navigation group via `@/modules/registry`.
- **Backend Boundary**: Communicates with the async-graphql workflow API via `graphqlRequest` with tenant and session token propagation.
- **Shared FFA Primitives**: Renders data using `@/widgets/data-table`, `@/shared/ui/shadcn/*`, and `@/shared/ui/module-unavailable`.
