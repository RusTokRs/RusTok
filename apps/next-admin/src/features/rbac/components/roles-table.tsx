'use client';

import { useTranslations } from 'next-intl';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from '@/widgets/data-table/table';
import { Badge } from '@/shared/ui/shadcn/badge';
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger
} from '@/shared/ui/shadcn/collapsible';
import { ChevronDown } from 'lucide-react';
import type { RoleInfo } from '../api/roles';

interface RolesTableProps {
  roles: RoleInfo[];
}

const ROLE_BADGE_VARIANT: Record<string, 'default' | 'secondary' | 'outline' | 'destructive'> = {
  super_admin: 'destructive',
  admin: 'default',
  manager: 'secondary',
  customer: 'outline'
};

export function RolesTable({ roles }: RolesTableProps) {
  const t = useTranslations('roles');

  return (
    <div className='rounded-md border'>
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead className='w-[180px]'>{t('list.role')}</TableHead>
            <TableHead className='w-[100px]'>{t('list.permissionsCount')}</TableHead>
            <TableHead>{t('list.permissionList')}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {roles.map((role) => (
            <TableRow key={role.slug}>
              <TableCell>
                <Badge variant={ROLE_BADGE_VARIANT[role.slug] ?? 'outline'}>
                  {role.displayName}
                </Badge>
              </TableCell>
              <TableCell className='text-muted-foreground'>
                {role.permissions.length}
              </TableCell>
              <TableCell>
                <Collapsible>
                  <CollapsibleTrigger className='flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground'>
                    {t('list.showPermissions')}
                    <ChevronDown className='h-3 w-3' />
                  </CollapsibleTrigger>
                  <CollapsibleContent>
                    <div className='mt-2 flex flex-wrap gap-1'>
                      {role.permissions.map((perm) => (
                        <Badge key={perm} variant='outline' className='text-xs font-mono'>
                          {perm}
                        </Badge>
                      ))}
                    </div>
                  </CollapsibleContent>
                </Collapsible>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}
