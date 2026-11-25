import {
  listPlatformPermissions,
  listRoles,
  type GqlOpts,
  type PlatformPermissionItem,
  type RoleInfo
} from '../api/roles';
import { RbacView } from '../components/rbac-view';

interface RolesPageProps {
  token?: string | null;
  tenantSlug?: string | null;
}

export default async function RolesPage({ token, tenantSlug }: RolesPageProps) {
  const opts: GqlOpts = { token, tenantSlug };
  const [rolesResult, catalogResult] = await Promise.allSettled([
    listRoles(opts),
    listPlatformPermissions(opts)
  ]);

  let roles: RoleInfo[] = [];
  let initialError: string | null = null;
  if (rolesResult.status === 'fulfilled') {
    roles = rolesResult.value;
  } else {
    initialError =
      rolesResult.reason instanceof Error
        ? rolesResult.reason.message
        : 'Failed to load RBAC roles';
  }

  let initialPlatformPermissions: PlatformPermissionItem[] = [];
  let initialPermissionsError: string | null = null;
  if (catalogResult.status === 'fulfilled') {
    initialPlatformPermissions = catalogResult.value;
  } else {
    initialPermissionsError =
      catalogResult.reason instanceof Error
        ? catalogResult.reason.message
        : 'Failed to load platform permissions catalog';
  }

  return (
    <RbacView
      roles={roles}
      initialPlatformPermissions={initialPlatformPermissions}
      initialError={initialError}
      initialPermissionsError={initialPermissionsError}
      token={token}
      tenantSlug={tenantSlug}
    />
  );
}
