import { listRoles, type GqlOpts, type RoleInfo } from '../api/roles';
import { RbacView } from '../components/rbac-view';

interface RolesPageProps {
  token?: string | null;
  tenantSlug?: string | null;
}

const FALLBACK_SYSTEM_ROLES: RoleInfo[] = [
  {
    slug: 'super_admin',
    displayName: 'Super Admin',
    isSystem: true,
    permissions: [
      'users:manage',
      'tenants:manage',
      'modules:manage',
      'settings:manage',
      'products:manage',
      'orders:manage',
      'inventory:manage',
      'content:manage',
      'workflows:manage',
      'ai:manage'
    ]
  },
  {
    slug: 'admin',
    displayName: 'Admin',
    isSystem: true,
    permissions: [
      'users:manage',
      'settings:manage',
      'products:manage',
      'orders:manage',
      'inventory:manage',
      'content:manage',
      'workflows:manage',
      'modules:read',
      'logs:read'
    ]
  },
  {
    slug: 'manager',
    displayName: 'Manager',
    isSystem: true,
    permissions: [
      'products:read',
      'products:create',
      'products:update',
      'orders:read',
      'orders:update',
      'inventory:read',
      'inventory:update',
      'content:manage'
    ]
  },
  {
    slug: 'customer',
    displayName: 'Customer',
    isSystem: true,
    permissions: [
      'products:read',
      'categories:read',
      'orders:read',
      'orders:create',
      'comments:create'
    ]
  }
];

export default async function RolesPage({ token, tenantSlug }: RolesPageProps) {
  const opts: GqlOpts = { token, tenantSlug };
  let roles: RoleInfo[] = [];

  try {
    roles = await listRoles(opts);
  } catch (error) {
    console.error('Failed to query RBAC roles from API, falling back to system defaults:', error);
    roles = FALLBACK_SYSTEM_ROLES;
  }

  return <RbacView roles={roles} token={token} tenantSlug={tenantSlug} />;
}
