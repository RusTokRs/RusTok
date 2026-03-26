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

export interface AssignUserRoleInput {
  userId: string;
  role: string;
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
  const data = await graphqlRequest<
    { input: AssignUserRoleInput },
    AssignUserRoleResponse
  >(ASSIGN_USER_ROLE_MUTATION, { input }, opts.token, opts.tenantSlug);
  return data.assignUserRole;
}
