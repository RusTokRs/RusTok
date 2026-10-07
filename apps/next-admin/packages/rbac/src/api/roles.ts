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
}

export interface PlatformPermissionItem {
  id: string;
  resource: string;
  action: string;
  description?: string | null;
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

export type UserRoleEnum = 'SUPER_ADMIN' | 'ADMIN' | 'MANAGER' | 'CUSTOMER';

export function roleSlugToUserRoleEnum(slug: string): UserRoleEnum {
  const normalized = slug.trim().toUpperCase();
  if (
    normalized === 'SUPER_ADMIN' ||
    normalized === 'ADMIN' ||
    normalized === 'MANAGER' ||
    normalized === 'CUSTOMER'
  ) {
    return normalized as UserRoleEnum;
  }
  return 'CUSTOMER';
}

export interface AssignUserRoleInput {
  userId: string;
  role: UserRoleEnum | string;
}

export interface AssignUserRolePayload {
  success: boolean;
  userId: string;
  role: string;
}

// ---------- GraphQL ----------

const ROLES_QUERY = `
query Roles {
  roles {
    id
    slug
    displayName
    description
    isSystem
    permissions
  }
}
`;

const PLATFORM_PERMISSIONS_QUERY = `
query PlatformPermissions {
  platformPermissions {
    id
    resource
    action
    description
  }
}
`;

const CREATE_ROLE_MUTATION = `
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
    }
  }
}
`;

const UPDATE_ROLE_MUTATION = `
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
    }
  }
}
`;

const DELETE_ROLE_MUTATION = `
mutation DeleteRole($slug: String!) {
  deleteRole(slug: $slug) {
    success
    slug
  }
}
`;

const ASSIGN_USER_ROLE_MUTATION = `
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
  const data = await graphqlRequest<
    { input: CreateRoleInput },
    CreateRoleResponse
  >(CREATE_ROLE_MUTATION, { input }, opts.token, opts.tenantSlug);
  return data.createRole;
}

export async function updateRole(
  input: UpdateRoleInput,
  opts: GqlOpts = {}
): Promise<RoleMutationPayload> {
  const data = await graphqlRequest<
    { input: UpdateRoleInput },
    UpdateRoleResponse
  >(UPDATE_ROLE_MUTATION, { input }, opts.token, opts.tenantSlug);
  return data.updateRole;
}

export async function deleteRole(
  slug: string,
  opts: GqlOpts = {}
): Promise<DeleteRolePayload> {
  const data = await graphqlRequest<{ slug: string }, DeleteRoleResponse>(
    DELETE_ROLE_MUTATION,
    { slug },
    opts.token,
    opts.tenantSlug
  );
  return data.deleteRole;
}

export async function assignUserRole(
  input: AssignUserRoleInput,
  opts: GqlOpts = {}
): Promise<AssignUserRolePayload> {
  const payload = {
    userId: input.userId,
    role: roleSlugToUserRoleEnum(input.role)
  };
  const data = await graphqlRequest<
    { input: { userId: string; role: UserRoleEnum } },
    AssignUserRoleResponse
  >(ASSIGN_USER_ROLE_MUTATION, { input: payload }, opts.token, opts.tenantSlug);
  return data.assignUserRole;
}
