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
import {
  MoreHorizontal,
  ExternalLink,
  UserCheck,
  Copy,
  Shield
} from 'lucide-react';
import Link from 'next/link';
import { useRouter } from 'next/navigation';
import { toast } from 'sonner';
import { AssignRoleDialog, listRoles, type RoleInfo } from '@rustok/rbac-admin';
import { useSession } from 'next-auth/react';
import { useTranslations } from '@rustok/next-fluent';
import type { User } from '@/entities/user';

interface CellActionProps {
  data: User;
  roles?: RoleInfo[];
}

const FALLBACK_ROLES: RoleInfo[] = [
  {
    slug: 'super_admin',
    displayName: 'Super Admin',
    permissions: [],
    isSystem: true
  },
  { slug: 'admin', displayName: 'Admin', permissions: [], isSystem: true },
  { slug: 'manager', displayName: 'Manager', permissions: [], isSystem: true },
  { slug: 'customer', displayName: 'Customer', permissions: [], isSystem: true }
];

export const CellAction: React.FC<CellActionProps> = ({ data, roles }) => {
  const t = useTranslations('users');
  const router = useRouter();
  const { data: session } = useSession();
  const token = session?.user?.rustokToken;
  const tenantSlug = session?.user?.tenantSlug;

  const [roleDialogOpen, setRoleDialogOpen] = React.useState(false);
  const [availableRoles, setAvailableRoles] = React.useState<RoleInfo[]>(
    roles && roles.length > 0 ? roles : FALLBACK_ROLES
  );

  React.useEffect(() => {
    if (roles && roles.length > 0) {
      setAvailableRoles(roles);
      return;
    }
    if (token) {
      listRoles({ token, tenantSlug })
        .then((fetched) => {
          if (fetched && fetched.length > 0) {
            setAvailableRoles(fetched);
          }
        })
        .catch(() => {
          // fallback roles remain
        });
    }
  }, [roles, token, tenantSlug]);

  const handleCopyId = () => {
    navigator.clipboard.writeText(data.id);
    toast.success(t('toast.id.copied'));
  };

  return (
    <>
      <DropdownMenu modal={false}>
        <DropdownMenuTrigger asChild>
          <Button variant='ghost' className='h-8 w-8 p-0'>
            <span className='sr-only'>{t('action.open.menu')}</span>
            <MoreHorizontal className='h-4 w-4' />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align='end'>
          <DropdownMenuLabel>{t('action.title')}</DropdownMenuLabel>
          <DropdownMenuItem onClick={handleCopyId}>
            <Copy className='mr-2 h-4 w-4' /> {t('action.copy.id')}
          </DropdownMenuItem>
          <DropdownMenuItem onClick={() => setRoleDialogOpen(true)}>
            <Shield className='mr-2 h-4 w-4' /> {t('action.change.role')}
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem asChild>
            <Link href={`/dashboard/users/${data.id}`}>
              <UserCheck className='mr-2 h-4 w-4' /> {t('action.view.profile')}
            </Link>
          </DropdownMenuItem>
          <DropdownMenuItem asChild>
            <Link href={`/dashboard/users/${data.id}`} target='_blank'>
              <ExternalLink className='mr-2 h-4 w-4' />{' '}
              {t('action.open.new.tab')}
            </Link>
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      <AssignRoleDialog
        open={roleDialogOpen}
        onOpenChange={setRoleDialogOpen}
        roles={availableRoles}
        defaultUserId={data.id}
        defaultRoleSlug={data.role?.toLowerCase()}
        onSuccess={() => {
          router.refresh();
        }}
      />
    </>
  );
};
