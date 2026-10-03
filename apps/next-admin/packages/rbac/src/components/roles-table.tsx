import { getTranslations } from '@rustok/next-fluent/server';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from '@/widgets/data-table';
import { Badge } from '@/shared/ui/shadcn/badge';
import { IconChevronDown } from '@tabler/icons-react';
import type { RoleInfo } from '../api/roles';

interface RolesTableProps {
  roles: RoleInfo[];
}

const ROLE_BADGE_VARIANT: Record<
  string,
  'default' | 'secondary' | 'outline' | 'destructive'
> = {
  super_admin: 'destructive',
  admin: 'default',
  manager: 'secondary',
  customer: 'outline'
};

export async function RolesTable({ roles }: RolesTableProps) {
  const t = await getTranslations('roles');




  return (
    <div className='rounded-md border'>
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead className='w-[180px]'>{t('list.role')}</TableHead>
            <TableHead className='w-[100px]'>
              {t('list.permissionsCount')}
            </TableHead>
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
                <details className='group'>
                  <summary className='text-muted-foreground hover:text-foreground flex items-center gap-1 text-sm cursor-pointer list-none'>
                    {t('list.showPermissions')}
                    <IconChevronDown className='h-3 w-3 transition-transform group-open:rotate-180' />
                  </summary>
                  <div className='mt-2 flex flex-wrap gap-1'>
                    {role.permissions.map((perm) => (
                      <Badge
                        key={perm}
                        variant='outline'
                        className='font-mono text-xs'
                      >
                        {perm}
                      </Badge>
                    ))}
                  </div>
                </details>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}
