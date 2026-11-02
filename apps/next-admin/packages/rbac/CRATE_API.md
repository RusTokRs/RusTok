# @rustok/rbac-admin — Canonical API Specification

## 1. Package Overview

`@rustok/rbac-admin` provides the module-owned Next.js administration interface for the RusToK Role-Based Access Control (RBAC) platform. It manages tenant roles, platform permission matrices, dormant grant policies, and user role assignments over the unified `rustok-rbac` async-graphql backend schema.

- **Package Name**: `@rustok/rbac-admin`
- **Location**: `apps/next-admin/packages/rbac`
- **Owner**: `rustok-rbac` module frontend surface
- **Status**: Stable / Verified

## 2. Public API Surface

### Exported Components & Pages

- `RolesPage` (default export & alias `RolesAdminPage`): Server component loading roles with system fallback and rendering the full RBAC view.
- `RbacView`: Client container managing tab switching (Roles, Permissions Table), metrics cards, dialog states, and dynamic permissions fetching.
- `RolesTable`: Data table displaying roles, system vs custom badges, permission totals, inactive counts, and expansion drawers.
- `PermissionsTable`: Filterable matrix table displaying all platform permissions, associated modules, and granted roles.
- `PermissionMatrix`: Interactive permission selector grouped by resource/module with search, select-all, and dormant grant safeguards.
- `CreateRoleDialog`: Dialog for defining new custom roles with name-derived slug generator and reserved slug protection.
- `EditRoleDialog`: Dialog for updating role names, descriptions, and custom permission sets (system super_admin permissions protected).
- `DeleteRoleDialog`: Confirmation dialog for role removal with hard protection for `super_admin`.
- `AssignRoleDialog`: Dialog for assigning roles to user UUIDs with strict format verification.

### Navigation

- `rbacNavItems: NavItem[]`: Declares `/dashboard/roles` under the `Access Control` navigation group with `['r', 'p']` keyboard shortcut and `{ role: 'admin' }` access policy.

### API & Data Fetching

- `ROLES_QUERY`: GraphQL query returning all roles with active and dormant permission lists.
- `PLATFORM_PERMISSIONS_QUERY`: GraphQL query retrieving all declared platform permissions and their module status (`core`, `enabled`, `disabled`).
- `CREATE_ROLE_MUTATION`: GraphQL mutation to create custom roles.
- `UPDATE_ROLE_MUTATION`: GraphQL mutation to update role metadata and permission grants.
- `DELETE_ROLE_MUTATION`: GraphQL mutation to delete a role by slug.
- `ASSIGN_USER_ROLE_MUTATION`: GraphQL mutation to associate a user account with a role.
- `listRoles(opts)`: Queries tenant roles with session auth context.
- `listPlatformPermissions(opts)`: Queries catalog of platform permissions.
- `createRole(input, opts)`: Validates name, slug (min 2 chars, non-reserved), and executes creation mutation.
- `updateRole(input, opts)`: Validates slug and executes role update.
- `deleteRole(slug, opts)`: Validates slug, protects `super_admin`, and executes deletion.
- `assignUserRole(input, opts)`: Validates user ID format (UUID v4) and assigns role.

## 3. Dependencies & Architecture Boundaries

- **Direct Dependencies**: `react: ^19.2.0`, `next: ^15.0.0` (peer / host).
- **Host Integration**: Registered in the Next.js admin registry via `registerAdminModule({ id: 'rbac', name: 'Access Control', navItems: rbacNavItems })`.
- **Backend Boundary**: Communicates with the `rustok-rbac` async-graphql schema via `graphqlRequest`.

## 4. Invariants & Security

- **Super Admin Immutability & Protection**: The `super_admin` role cannot be deleted via `deleteRole` or have its permissions revoked via `updateRole`.
- **Reserved Built-in Slugs**: Custom roles cannot use reserved system slugs (`super_admin`, `admin`, `manager`, `customer`).
- **Dormant Grant Integrity**: Permissions belonging to disabled tenant modules (`moduleStatus === 'disabled'`) can be removed from a role but cannot be newly granted while the module remains disabled.
- **Fail-Closed Input Validation**: Non-empty role names and slugs (>= 2 characters), as well as RFC 4122 UUID validation on user assignments, are enforced at the API boundary before issuing network requests.

## 5. Verification & Testing

- **Contract Tests**: `packages/rbac/src/rbac-admin.test.mjs` executed via `npm --prefix apps/next-admin run test:rbac`.
- **Verified Coverage**:
  - `rbacNavItems` routing, access control (`role: 'admin'`), and shortcuts (`r, p`).
  - GraphQL documents (`ROLES_QUERY`, `PLATFORM_PERMISSIONS_QUERY`, mutations).
  - `createRole` input sanitization, length rules, and reserved slug rejection.
  - `deleteRole` protection against deleting `super_admin`.
  - `assignUserRole` UUID validation and parameter propagation.
  - `updateRole` parameter handling and `super_admin` permission immutability.
