'use client';

import * as React from 'react';
import { Button } from '@/shared/ui/shadcn/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger
} from '@/shared/ui/shadcn/dropdown-menu';
import { MoreHorizontal, ExternalLink, UserCheck, Copy, Shield } from 'lucide-react';
import Link from 'next/link';
import { toast } from 'sonner';
import { AssignRoleDialog } from '@rustok/rbac-admin';
import type { User } from '@/entities/user';

interface CellActionProps {
  data: User;
}

const PLATFORM_ROLES = [
  { slug: 'super_admin', displayName: 'Super Admin', permissions: [] },
  { slug: 'admin', displayName: 'Admin', permissions: [] },
  { slug: 'manager', displayName: 'Manager', permissions: [] },
  { slug: 'customer', displayName: 'Customer', permissions: [] }
];

export const CellAction: React.FC<CellActionProps> = ({ data }) => {
  const [roleDialogOpen, setRoleDialogOpen] = React.useState(false);

  const handleCopyId = () => {
    navigator.clipboard.writeText(data.id);
    toast.success('User ID copied to clipboard');
  };

  return (
    <>
      <DropdownMenu modal={false}>
        <DropdownMenuTrigger asChild>
          <Button variant='ghost' className='h-8 w-8 p-0'>
            <span className='sr-only'>Open menu</span>
            <MoreHorizontal className='h-4 w-4' />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align='end'>
          <DropdownMenuLabel>Actions</DropdownMenuLabel>
          <DropdownMenuItem onClick={handleCopyId}>
            <Copy className='mr-2 h-4 w-4' /> Copy User ID
          </DropdownMenuItem>
          <DropdownMenuItem onClick={() => setRoleDialogOpen(true)}>
            <Shield className='mr-2 h-4 w-4' /> Change Role
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem asChild>
            <Link href={`/dashboard/users/${data.id}`}>
              <UserCheck className='mr-2 h-4 w-4' /> View Profile
            </Link>
          </DropdownMenuItem>
          <DropdownMenuItem asChild>
            <Link href={`/dashboard/users/${data.id}`} target='_blank'>
              <ExternalLink className='mr-2 h-4 w-4' /> Open in new tab
            </Link>
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      <AssignRoleDialog
        open={roleDialogOpen}
        onOpenChange={setRoleDialogOpen}
        roles={PLATFORM_ROLES}
        defaultUserId={data.id}
        defaultRoleSlug={data.role?.toLowerCase()}
        onSuccess={() => {
          // If in a router, triggers refresh
          window.location.reload();
        }}
      />
    </>
  );
};

