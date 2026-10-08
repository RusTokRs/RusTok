import type { RoleInfo } from './api/roles';

export interface PermissionRecord {
  slug: string;
  module: string;
  action: string;
  roles: string[];
}

export type RoleBadgeVariant =
  'default' | 'secondary' | 'outline' | 'destructive';

export interface RbacStats {
  totalRoles: number;
  totalPermissions: number;
  adminRolesCount: number;
  standardRolesCount: number;
}
