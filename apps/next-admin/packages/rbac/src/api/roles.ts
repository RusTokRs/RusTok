import { graphqlRequest } from '@/lib/graphql';

export interface GqlOpts {
  token?: string | null;
  tenantSlug?: string | null;
}

// ---------- Types ----------

export interface RoleInfo {
  id?: string | null;
  slug: string;
  displayName: string;
  description?: string | null;
  isSystem?: boolean;
  permissions: string[];
  /** Grants whose owning module is disabled for this tenant: persisted but not effective. */
  inactivePermissions?: string[];
}

export interface PlatformPermissionItem {
  id: string;
  resource: string;
  action: string;
  description?: string | null;
  /** Owning module slug when a module declares this permission. */
  moduleSlug?: string | null;
  /** Availability for this tenant: 'core' | 'enabled' | 'disabled'. */
  moduleStatus?: string;
}

export interface CreateRoleInput {
  name: string;
  slug: string;
  description?: string | null;
  permissions: string[];
}

export interface UpdateRoleInput {
  slug: string;
  name?: string | null;
  description?: string | null;
  permissions?: string[] | null;
}

export interface RoleMutationPayload {
  success: boolean;
  role?: RoleInfo | null;
}

export interface DeleteRolePayload {
  success: boolean;
  slug: string;
}

export interface AssignUserRoleInput {
  userId: string;
  /** Slug of any tenant role (built-in or custom). */
  roleSlug: string;
}

export interface AssignUserRolePayload {
  success: boolean;
  userId: string;
  role: string;
}

// ---------- GraphQL ----------

export const ROLES_QUERY = `
query Roles {
  roles {
    id
    slug
    displayName
    description
    isSystem
    permissions
    inactivePermissions
  }
}
`;

export const PLATFORM_PERMISSIONS_QUERY = `
query PlatformPermissions {
  platformPermissions {
    id
    resource
    action
    description
    moduleSlug
    moduleStatus
  }
}
`;

export const CREATE_ROLE_MUTATION = `
mutation CreateRole($input: CreateRoleInput!) {
  createRole(input: $input) {
    success
    role {
      id
      slug
      displayName
      description
      isSystem
      permissions
      inactivePermissions
    }
  }
}
`;

export const UPDATE_ROLE_MUTATION = `
mutation UpdateRole($input: UpdateRoleInput!) {
  updateRole(input: $input) {
    success
    role {
      id
      slug
      displayName
      description
      isSystem
      permissions
      inactivePermissions
    }
  }
}
`;

export const DELETE_ROLE_MUTATION = `
mutation DeleteRole($slug: String!) {
  deleteRole(slug: $slug) {
    success
    slug
  }
}
`;

export const ASSIGN_USER_ROLE_MUTATION = `
mutation AssignUserRole($input: AssignUserRoleInput!) {
  assignUserRole(input: $input) {
    success
    userId
    role
  }
}
`;

// ---------- API functions ----------

interface RolesQueryResponse {
  roles: RoleInfo[];
}

interface PlatformPermissionsResponse {
  platformPermissions: PlatformPermissionItem[];
}

interface CreateRoleResponse {
  createRole: RoleMutationPayload;
}

interface UpdateRoleResponse {
  updateRole: RoleMutationPayload;
}

interface DeleteRoleResponse {
  deleteRole: DeleteRolePayload;
}

interface AssignUserRoleResponse {
  assignUserRole: AssignUserRolePayload;
}

export async function listRoles(opts: GqlOpts = {}): Promise<RoleInfo[]> {
  const data = await graphqlRequest<Record<string, never>, RolesQueryResponse>(
    ROLES_QUERY,
    {},
    opts.token,
    opts.tenantSlug
  );
  return data.roles;
}

export async function listPlatformPermissions(
  opts: GqlOpts = {}
): Promise<PlatformPermissionItem[]> {
  const data = await graphqlRequest<
    Record<string, never>,
    PlatformPermissionsResponse
  >(PLATFORM_PERMISSIONS_QUERY, {}, opts.token, opts.tenantSlug);
  return data.platformPermissions;
}

export async function createRole(
  input: CreateRoleInput,
  opts: GqlOpts = {}
): Promise<RoleMutationPayload> {
  const cleanName = input.name?.trim();
  if (!cleanName) {
    throw new Error('Role name is required');
  }
  const cleanSlug = input.slug?.trim().toLowerCase();
  if (!cleanSlug || cleanSlug.length < 2) {
    throw new Error('Role slug must be at least 2 characters');
  }
  const reserved = ['super_admin', 'admin', 'manager', 'customer'];
  if (reserved.includes(cleanSlug)) {
    throw new Error(`'${cleanSlug}' is a reserved platform built-in role slug`);
  }
  const payload: CreateRoleInput = {
    name: cleanName,
    slug: cleanSlug,
    description: input.description?.trim() || null,
    permissions: input.permissions ?? []
  };
  const data = await graphqlRequest<
    { input: CreateRoleInput },
    CreateRoleResponse
  >(CREATE_ROLE_MUTATION, { input: payload }, opts.token, opts.tenantSlug);
  return data.createRole;
}

export async function updateRole(
  input: UpdateRoleInput,
  opts: GqlOpts = {}
): Promise<RoleMutationPayload> {
  const cleanSlug = input.slug?.trim().toLowerCase();
  if (!cleanSlug) {
    throw new Error('Role slug is required');
  }
  const payload: UpdateRoleInput = {
    slug: cleanSlug,
    name: input.name !== undefined ? input.name?.trim() || null : undefined,
    description:
      input.description !== undefined
        ? input.description?.trim() || null
        : undefined,
    permissions:
      cleanSlug === 'super_admin' ? undefined : (input.permissions ?? undefined)
  };
  const data = await graphqlRequest<
    { input: UpdateRoleInput },
    UpdateRoleResponse
  >(UPDATE_ROLE_MUTATION, { input: payload }, opts.token, opts.tenantSlug);
  return data.updateRole;
}

export async function deleteRole(
  slug: string,
  opts: GqlOpts = {}
): Promise<DeleteRolePayload> {
  const cleanSlug = slug?.trim().toLowerCase();
  if (!cleanSlug) {
    throw new Error('Role slug is required');
  }
  if (cleanSlug === 'super_admin') {
    throw new Error("Cannot delete protected system role 'super_admin'");
  }
  const data = await graphqlRequest<{ slug: string }, DeleteRoleResponse>(
    DELETE_ROLE_MUTATION,
    { slug: cleanSlug },
    opts.token,
    opts.tenantSlug
  );
  return data.deleteRole;
}

export async function assignUserRole(
  input: AssignUserRoleInput,
  opts: GqlOpts = {}
): Promise<AssignUserRolePayload> {
  const cleanUserId = input.userId?.trim();
  if (!cleanUserId) {
    throw new Error('User ID is required');
  }
  const uuidRegex =
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
  if (!uuidRegex.test(cleanUserId)) {
    throw new Error('User ID must be a valid UUID');
  }
  const cleanRoleSlug = input.roleSlug?.trim().toLowerCase();
  if (!cleanRoleSlug) {
    throw new Error('Role slug is required');
  }
  const payload = {
    userId: cleanUserId,
    roleSlug: cleanRoleSlug
  };
  const data = await graphqlRequest<
    { input: { userId: string; roleSlug: string } },
    AssignUserRoleResponse
  >(ASSIGN_USER_ROLE_MUTATION, { input: payload }, opts.token, opts.tenantSlug);
  return data.assignUserRole;
}
