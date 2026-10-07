import { registerAdminModule } from '@/modules/registry';
import { rbacNavItems } from './nav';

registerAdminModule({
  id: 'rbac',
  name: 'Access Control',
  navItems: rbacNavItems
});

export { rbacNavItems } from './nav';
export { default as RolesPage } from './pages/roles-page';
export { RolesTable } from './components/roles-table';
export { PermissionsTable } from './components/permissions-table';
export { AssignRoleDialog } from './components/assign-role-dialog';
export { RbacView } from './components/rbac-view';
export * from './api/roles';
export * from './types';
