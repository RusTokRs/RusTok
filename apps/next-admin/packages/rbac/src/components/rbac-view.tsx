'use client';

import * as React from 'react';
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger
} from '@/shared/ui/shadcn/tabs';
import { Button } from '@/shared/ui/shadcn/button';
import { IconShieldCheck, IconKey, IconUserPlus } from '@tabler/icons-react';
import type { RoleInfo } from '../api/roles';
import { RolesTable } from './roles-table';
import { PermissionsTable } from './permissions-table';
import { AssignRoleDialog } from './assign-role-dialog';

interface RbacViewProps {
  roles: RoleInfo[];
  token?: string | null;
  tenantSlug?: string | null;
}

export function RbacView({ roles, token, tenantSlug }: RbacViewProps) {
  const [assignDialogOpen, setAssignDialogOpen] = React.useState(false);
  const [selectedRoleSlug, setSelectedRoleSlug] = React.useState<string | undefined>();

  const handleOpenAssign = (roleSlug?: string) => {
    setSelectedRoleSlug(roleSlug);
    setAssignDialogOpen(true);
  };

  return (
    <div className='space-y-6'>
      <Tabs defaultValue='roles' className='w-full'>
        <div className='flex flex-col sm:flex-row sm:items-center justify-between gap-4 pb-2'>
          <TabsList>
            <TabsTrigger value='roles' className='flex items-center gap-2'>
              <IconShieldCheck className='h-4 w-4' />
              <span>Roles ({roles.length})</span>
            </TabsTrigger>
            <TabsTrigger value='permissions' className='flex items-center gap-2'>
              <IconKey className='h-4 w-4' />
              <span>Permissions Catalog</span>
            </TabsTrigger>
          </TabsList>

          <Button
            size='sm'
            onClick={() => handleOpenAssign()}
            className='flex items-center gap-1.5'
          >
            <IconUserPlus className='h-4 w-4' />
            <span>Assign Role to User</span>
          </Button>
        </div>

        <TabsContent value='roles' className='mt-4 space-y-4'>
          <RolesTable
            roles={roles}
            onAssignRole={(slug) => handleOpenAssign(slug)}
          />
        </TabsContent>

        <TabsContent value='permissions' className='mt-4 space-y-4'>
          <PermissionsTable roles={roles} />
        </TabsContent>
      </Tabs>

      <AssignRoleDialog
        open={assignDialogOpen}
        onOpenChange={setAssignDialogOpen}
        roles={roles}
        defaultRoleSlug={selectedRoleSlug}
        token={token}
        tenantSlug={tenantSlug}
      />
    </div>
  );
}
