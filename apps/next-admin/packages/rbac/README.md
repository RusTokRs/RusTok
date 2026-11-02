# @rustok/rbac-admin

Module-owned Next.js administration package for the RusToK Role-Based Access Control (RBAC) system.

## Overview

This package provides the administration interface for tenant roles, permissions matrix, and role assignments:
- **Roles Management**: View system and custom roles, grant counts, descriptions, and active/dormant status.
- **Permission Matrix**: Inspect and edit granular permissions grouped by domain module, respecting dormant grant locks when modules are inactive.
- **User Role Assignments**: Assign tenant roles to user accounts with fail-closed UUID validation.
- **System Safeguards**: Protects system built-in roles (`super_admin`, `admin`, `manager`, `customer`) against unauthorized creation and prevents deletion of the core `super_admin` identity.

## Architecture

- **Host Integration**: Registers `/dashboard/roles` under the `Access Control` navigation group via `@/modules/registry`.
- **Backend Boundary**: Communicates with the async-graphql API using `graphqlRequest` with tenant and session token propagation.
- **Shared FFA Primitives**: Renders data using `@/widgets/data-table` and `@rustok/next-fluent` i18n catalogs.
