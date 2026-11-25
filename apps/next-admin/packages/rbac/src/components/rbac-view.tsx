'use client';

import * as React from 'react';
import { useTranslations } from '@rustok/next-fluent/client';
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger
} from '@/shared/ui/shadcn/tabs';
import {
  Alert,
  AlertDescription,
  AlertTitle
} from '@/shared/ui/shadcn/alert';
import { Button } from '@/shared/ui/shadcn/button';
import {
  IconShieldCheck,
  IconKey,
  IconUserPlus,
  IconPlus,
  IconAlertTriangle
} from '@tabler/icons-react';
import {
  listPlatformPermissions,
  listRoles,
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
  initialPlatformPermissions?: PlatformPermissionItem[];
  initialError?: string | null;
  initialPermissionsError?: string | null;
  token?: string | null;
  tenantSlug?: string | null;
}

export function RbacView({
  roles: initialRoles,
  initialPlatformPermissions = [],
  initialError = null,
  initialPermissionsError = null,
  token,
  tenantSlug
}: RbacViewProps) {
  const t = useTranslations('rbac');
  const [roles, setRoles] = React.useState<RoleInfo[]>(initialRoles);
  const [rolesError, setRolesError] = React.useState<string | null>(
    initialError
  );
  const [platformPermissions, setPlatformPermissions] = React.useState<
    PlatformPermissionItem[]
  >(initialPlatformPermissions);
  const [permissionsError, setPermissionsError] = React.useState<string | null>(
    initialPermissionsError
  );
  const [retrying, setRetrying] = React.useState(false);

  // Dialog states
  const [createDialogOpen, setCreateDialogOpen] = React.useState(false);
  const [editDialogOpen, setEditDialogOpen] = React.useState(false);
  const [deleteDialogOpen, setDeleteDialogOpen] = React.useState(false);
  const [assignDialogOpen, setAssignDialogOpen] = React.useState(false);

  const [selectedRole, setSelectedRole] = React.useState<RoleInfo | null>(null);
  const [assignRoleSlug, setAssignRoleSlug] = React.useState<
    string | undefined
  >();

  // Fetch platform permissions on mount if not preloaded by the server component
  React.useEffect(() => {
    if (initialPlatformPermissions.length > 0 && !initialPermissionsError) {
      return;
    }
    let active = true;
    listPlatformPermissions({ token, tenantSlug })
      .then((items) => {
        if (active) {
          setPlatformPermissions(items ?? []);
          setPermissionsError(null);
        }
      })
      .catch((err) => {
        if (active) {
          setPermissionsError(
            err instanceof Error
              ? err.message
              : 'Failed to load platform permissions catalog'
          );
        }
      });
    return () => {
      active = false;
    };
  }, [
    token,
    tenantSlug,
    initialPlatformPermissions.length,
    initialPermissionsError
  ]);

  const handleRetry = async () => {
    setRetrying(true);
    try {
      const [rolesRes, catalogRes] = await Promise.allSettled([
        listRoles({ token, tenantSlug }),
        listPlatformPermissions({ token, tenantSlug })
      ]);
      if (rolesRes.status === 'fulfilled') {
        setRoles(rolesRes.value);
        setRolesError(null);
      } else {
        setRolesError(
          rolesRes.reason instanceof Error
            ? rolesRes.reason.message
            : 'Failed to load RBAC roles'
        );
      }
      if (catalogRes.status === 'fulfilled') {
        setPlatformPermissions(catalogRes.value ?? []);
        setPermissionsError(null);
      } else {
        setPermissionsError(
          catalogRes.reason instanceof Error
            ? catalogRes.reason.message
            : 'Failed to load platform permissions catalog'
        );
      }
    } finally {
      setRetrying(false);
    }
  };

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
      {rolesError && (
        <Alert variant='destructive'>
          <IconAlertTriangle className='h-4 w-4' />
          <AlertTitle>{t('load-error-title')}</AlertTitle>
          <AlertDescription className='flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between'>
            <span>
              {t('load-error-desc')} ({rolesError})
            </span>
            <Button
              size='sm'
              variant='outline'
              onClick={handleRetry}
              disabled={retrying}
            >
              {t('btn-retry')}
            </Button>
          </AlertDescription>
        </Alert>
      )}

      {!rolesError && permissionsError && (
        <Alert variant='destructive'>
          <IconAlertTriangle className='h-4 w-4' />
          <AlertTitle>{t('catalog-load-error')}</AlertTitle>
          <AlertDescription className='flex items-center justify-between gap-3'>
            <span>{permissionsError}</span>
            <Button
              size='sm'
              variant='outline'
              onClick={handleRetry}
              disabled={retrying}
            >
              {t('btn-retry')}
            </Button>
          </AlertDescription>
        </Alert>
      )}

      <Tabs defaultValue='roles' className='w-full'>
        <div className='flex flex-col justify-between gap-4 pb-2 sm:flex-row sm:items-center'>
          <TabsList>
            <TabsTrigger value='roles' className='flex items-center gap-2'>
              <IconShieldCheck className='h-4 w-4' />
              <span>{t('tab-roles', { count: roles.length })}</span>
            </TabsTrigger>
            <TabsTrigger
              value='permissions'
              className='flex items-center gap-2'
            >
              <IconKey className='h-4 w-4' />
              <span>{t('tab-permissions')}</span>
            </TabsTrigger>
          </TabsList>

          <div className='flex items-center gap-2'>
            <Button
              size='sm'
              variant='outline'
              disabled={Boolean(rolesError)}
              onClick={() => handleOpenAssign()}
              className='flex items-center gap-1.5'
            >
              <IconUserPlus className='h-4 w-4' />
              <span>{t('btn-assign-role')}</span>
            </Button>
            <Button
              size='sm'
              disabled={Boolean(rolesError)}
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
          <PermissionsTable
            roles={roles}
            platformPermissions={platformPermissions}
          />
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
