'use client';

import * as React from 'react';
import { useTranslations } from '@rustok/next-fluent';
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger
} from '@/shared/ui/shadcn/tabs';
import { Button } from '@/shared/ui/shadcn/button';
import { IconShieldCheck, IconKey, IconUserPlus, IconPlus } from '@tabler/icons-react';
import {
  listPlatformPermissions,
  type PlatformPermissionItem,
  type RoleInfo
} from '../api/roles';
import { RolesTable } from './roles-table';
import { PermissionsTable } from './permissions-table';
import { AssignRoleDialog } from './assign-role-dialog';
import { CreateRoleDialog } from './create-role-dialog';
import { EditRoleDialog } from './edit-role-dialog';
import { DeleteRoleDialog } from './delete-role-dialog';

interface RbacViewProps {
  roles: RoleInfo[];
  token?: string | null;
  tenantSlug?: string | null;
}

export function RbacView({ roles: initialRoles, token, tenantSlug }: RbacViewProps) {
  const t = useTranslations('rbac');
  const [roles, setRoles] = React.useState<RoleInfo[]>(initialRoles);
  const [platformPermissions, setPlatformPermissions] = React.useState<
    PlatformPermissionItem[]
  >([]);

  // Dialog states
  const [createDialogOpen, setCreateDialogOpen] = React.useState(false);
  const [editDialogOpen, setEditDialogOpen] = React.useState(false);
  const [deleteDialogOpen, setDeleteDialogOpen] = React.useState(false);
  const [assignDialogOpen, setAssignDialogOpen] = React.useState(false);

  const [selectedRole, setSelectedRole] = React.useState<RoleInfo | null>(null);
  const [assignRoleSlug, setAssignRoleSlug] = React.useState<string | undefined>();

  // Fetch all platform permissions on mount
  React.useEffect(() => {
    let active = true;
    listPlatformPermissions({ token, tenantSlug })
      .then((items) => {
        if (active && items) {
          setPlatformPermissions(items);
        }
      })
      .catch((err) => {
        console.error('Failed to load platform permissions catalog:', err);
      });
    return () => {
      active = false;
    };
  }, [token, tenantSlug]);

  const handleRoleCreated = (newRole: RoleInfo) => {
    setRoles((prev) => [newRole, ...prev]);
  };

  const handleRoleUpdated = (updatedRole: RoleInfo) => {
    setRoles((prev) =>
      prev.map((r) => (r.slug === updatedRole.slug ? updatedRole : r))
    );
  };

  const handleRoleDeleted = (deletedSlug: string) => {
    setRoles((prev) => prev.filter((r) => r.slug !== deletedSlug));
  };

  const handleOpenEdit = (role: RoleInfo) => {
    setSelectedRole(role);
    setEditDialogOpen(true);
  };

  const handleOpenDelete = (role: RoleInfo) => {
    setSelectedRole(role);
    setDeleteDialogOpen(true);
  };

  const handleOpenAssign = (roleSlug?: string) => {
    setAssignRoleSlug(roleSlug);
    setAssignDialogOpen(true);
  };

  return (
    <div className='space-y-6'>
      <Tabs defaultValue='roles' className='w-full'>
        <div className='flex flex-col sm:flex-row sm:items-center justify-between gap-4 pb-2'>
          <TabsList>
            <TabsTrigger value='roles' className='flex items-center gap-2'>
              <IconShieldCheck className='h-4 w-4' />
              <span>{t('tab-roles', { count: roles.length })}</span>
            </TabsTrigger>
            <TabsTrigger value='permissions' className='flex items-center gap-2'>
              <IconKey className='h-4 w-4' />
              <span>{t('tab-permissions')}</span>
            </TabsTrigger>
          </TabsList>

          <div className='flex items-center gap-2'>
            <Button
              size='sm'
              variant='outline'
              onClick={() => handleOpenAssign()}
              className='flex items-center gap-1.5'
            >
              <IconUserPlus className='h-4 w-4' />
              <span>{t('btn-assign-role')}</span>
            </Button>
            <Button
              size='sm'
              onClick={() => setCreateDialogOpen(true)}
              className='flex items-center gap-1.5'
            >
              <IconPlus className='h-4 w-4' />
              <span>{t('btn-create-role')}</span>
            </Button>
          </div>
        </div>

        <TabsContent value='roles' className='mt-4 space-y-4'>
          <RolesTable
            roles={roles}
            onAssignRole={(slug) => handleOpenAssign(slug)}
            onEditRole={handleOpenEdit}
            onDeleteRole={handleOpenDelete}
            onCreateRole={() => setCreateDialogOpen(true)}
          />
        </TabsContent>

        <TabsContent value='permissions' className='mt-4 space-y-4'>
          <PermissionsTable roles={roles} />
        </TabsContent>
      </Tabs>

      {/* Create Custom Role Dialog */}
      <CreateRoleDialog
        open={createDialogOpen}
        onOpenChange={setCreateDialogOpen}
        platformPermissions={platformPermissions}
        onCreated={handleRoleCreated}
        token={token}
        tenantSlug={tenantSlug}
      />

      {/* Edit Role & Permissions Dialog */}
      <EditRoleDialog
        role={selectedRole}
        open={editDialogOpen}
        onOpenChange={setEditDialogOpen}
        platformPermissions={platformPermissions}
        onUpdated={handleRoleUpdated}
        token={token}
        tenantSlug={tenantSlug}
      />

      {/* Delete Role Confirmation Dialog */}
      <DeleteRoleDialog
        role={selectedRole}
        open={deleteDialogOpen}
        onOpenChange={setDeleteDialogOpen}
        onDeleted={handleRoleDeleted}
        token={token}
        tenantSlug={tenantSlug}
      />

      {/* Assign Role to User Dialog */}
      <AssignRoleDialog
        open={assignDialogOpen}
        onOpenChange={setAssignDialogOpen}
        roles={roles}
        defaultRoleSlug={assignRoleSlug}
        token={token}
        tenantSlug={tenantSlug}
      />
    </div>
  );
}
