import { graphqlRequest } from '@/lib/graphql';

export interface GqlOpts {
  token?: string | null;
  tenantSlug?: string | null;
}

// ---------- Types ----------

export interface RoleInfo {
  slug: string;
  displayName: string;
  permissions: string[];
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
    slug
    displayName
    permissions
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
