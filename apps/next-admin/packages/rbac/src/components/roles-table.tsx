'use client';

import * as React from 'react';
import { useTranslations } from '@rustok/next-fluent';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from '@/widgets/data-table';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import {
  IconChevronDown,
  IconSearch,
  IconShieldLock,
  IconUserCheck,
  IconUsers,
  IconKey,
  IconPlus,
  IconEdit,
  IconTrash,
  IconLock
} from '@tabler/icons-react';
import type { RoleInfo } from '../api/roles';

interface RolesTableProps {
  roles: RoleInfo[];
  onAssignRole?: (roleSlug: string) => void;
  onEditRole?: (role: RoleInfo) => void;
  onDeleteRole?: (role: RoleInfo) => void;
  onCreateRole?: () => void;
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

export function RolesTable({
  roles,
  onAssignRole,
  onEditRole,
  onDeleteRole,
  onCreateRole
}: RolesTableProps) {
  const t = useTranslations('rbac');
  const [search, setSearch] = React.useState('');
  const [expandedRole, setExpandedRole] = React.useState<string | null>(null);

  // Compute summary metrics
  const totalUniquePermissions = React.useMemo(() => {
    const set = new Set<string>();
    for (const role of roles) {
      for (const p of role.permissions) {
        set.add(p);
      }
    }
    return set.size;
  }, [roles]);

  const customRolesCount = React.useMemo(() => {
    return roles.filter(
      (r) =>
        !r.isSystem &&
        !['super_admin', 'admin', 'manager', 'customer'].includes(r.slug)
    ).length;
  }, [roles]);

  const filteredRoles = React.useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return roles;
    return roles.filter(
      (role) =>
        role.displayName.toLowerCase().includes(q) ||
        role.slug.toLowerCase().includes(q) ||
        (role.description && role.description.toLowerCase().includes(q)) ||
        role.permissions.some((p) => p.toLowerCase().includes(q))
    );
  }, [roles, search]);

  return (
    <div className='space-y-6'>
      {/* Metrics Row */}
      <div className='grid gap-4 sm:grid-cols-2 lg:grid-cols-4'>
        <Card className='shadow-none border'>
          <CardHeader className='flex flex-row items-center justify-between pb-2 space-y-0'>
            <CardTitle className='text-xs font-medium text-muted-foreground'>
              {t('metrics-total-roles')}
            </CardTitle>
            <IconShieldLock className='h-4 w-4 text-primary' />
          </CardHeader>
          <CardContent>
            <div className='text-2xl font-bold'>{roles.length}</div>
            <p className='text-xs text-muted-foreground mt-1'>
              {t('metrics-total-roles-desc')}
            </p>
          </CardContent>
        </Card>

        <Card className='shadow-none border'>
          <CardHeader className='flex flex-row items-center justify-between pb-2 space-y-0'>
            <CardTitle className='text-xs font-medium text-muted-foreground'>
              {t('metrics-permissions')}
            </CardTitle>
            <IconKey className='h-4 w-4 text-emerald-600' />
          </CardHeader>
          <CardContent>
            <div className='text-2xl font-bold'>{totalUniquePermissions}</div>
            <p className='text-xs text-muted-foreground mt-1'>
              {t('metrics-permissions-desc')}
            </p>
          </CardContent>
        </Card>

        <Card className='shadow-none border'>
          <CardHeader className='flex flex-row items-center justify-between pb-2 space-y-0'>
            <CardTitle className='text-xs font-medium text-muted-foreground'>
              {t('metrics-admin-roles')}
            </CardTitle>
            <IconUserCheck className='h-4 w-4 text-amber-600' />
          </CardHeader>
          <CardContent>
            <div className='text-2xl font-bold'>
              {
                roles.filter(
                  (r) => r.slug === 'super_admin' || r.slug === 'admin'
                ).length
              }
            </div>
            <p className='text-xs text-muted-foreground mt-1'>
              {t('metrics-admin-roles-desc')}
            </p>
          </CardContent>
        </Card>

        <Card className='shadow-none border'>
          <CardHeader className='flex flex-row items-center justify-between pb-2 space-y-0'>
            <CardTitle className='text-xs font-medium text-muted-foreground'>
              {t('metrics-custom-roles')}
            </CardTitle>
            <IconUsers className='h-4 w-4 text-sky-600' />
          </CardHeader>
          <CardContent>
            <div className='text-2xl font-bold'>{customRolesCount}</div>
            <p className='text-xs text-muted-foreground mt-1'>
              {t('metrics-custom-roles-desc')}
            </p>
          </CardContent>
        </Card>
      </div>

      {/* Search & Actions Bar */}
      <div className='flex items-center justify-between gap-4'>
        <div className='relative flex-1 max-w-sm'>
          <IconSearch className='absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground' />
          <Input
            placeholder={t('search-placeholder')}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className='pl-9'
          />
        </div>

        {onCreateRole && (
          <Button
            size='sm'
            onClick={onCreateRole}
            className='flex items-center gap-1.5'
          >
            <IconPlus className='h-4 w-4' />
            <span>{t('btn-create-role')}</span>
          </Button>
        )}
      </div>

      {/* Main Table */}
      <div className='rounded-xl border bg-card shadow-sm overflow-hidden'>
        <Table>
          <TableHeader>
            <TableRow className='bg-muted/50'>
              <TableHead className='w-[200px]'>{t('table-role')}</TableHead>
              <TableHead className='w-[130px]'>{t('table-slug')}</TableHead>
              <TableHead className='w-[100px]'>{t('table-type')}</TableHead>
              <TableHead className='w-[110px]'>{t('table-permissions')}</TableHead>
              <TableHead className='min-w-[260px]'>{t('table-description')}</TableHead>
              <TableHead className='w-[220px] text-right'>{t('table-actions')}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {filteredRoles.map((role) => {
              const isExpanded = expandedRole === role.slug;
              const isSystem =
                Boolean(role.isSystem) ||
                ['super_admin', 'admin', 'manager', 'customer'].includes(role.slug);
              const description =
                role.description ||
                (role.slug === 'super_admin'
                  ? t('role-desc-super-admin')
                  : role.slug === 'admin'
                    ? t('role-desc-admin')
                    : role.slug === 'manager'
                      ? t('role-desc-manager')
                      : role.slug === 'customer'
                        ? t('role-desc-customer')
                        : t('role-desc-custom'));

              return (
                <React.Fragment key={role.slug}>
                  <TableRow className='hover:bg-muted/40'>
                    <TableCell>
                      <div className='flex items-center gap-2'>
                        <Badge variant={ROLE_BADGE_VARIANT[role.slug] ?? 'outline'}>
                          {role.displayName}
                        </Badge>
                      </div>
                    </TableCell>
                    <TableCell>
                      <span className='font-mono text-xs text-muted-foreground'>
                        {role.slug}
                      </span>
                    </TableCell>
                    <TableCell>
                      <Badge
                        variant={isSystem ? 'secondary' : 'outline'}
                        className='text-[10px] font-medium'
                      >
                        {isSystem ? t('badge-system') : t('badge-custom')}
                      </Badge>
                    </TableCell>
                    <TableCell>
                      <Badge variant='secondary' className='font-mono text-xs'>
                        {t('perms-total', { count: role.permissions.length })}
                      </Badge>
                    </TableCell>
                    <TableCell>
                      <div className='space-y-1.5'>
                        <p className='text-xs text-muted-foreground line-clamp-1'>
                          {description}
                        </p>
                        <button
                          type='button'
                          onClick={() =>
                            setExpandedRole(isExpanded ? null : role.slug)
                          }
                          className='text-muted-foreground hover:text-foreground flex items-center gap-1 text-xs cursor-pointer font-medium transition-colors'
                        >
                          {isExpanded
                            ? t('action-hide-perms')
                            : t('action-view-perms', { count: role.permissions.length })}
                          <IconChevronDown
                            className={`h-3 w-3 transition-transform ${
                              isExpanded ? 'rotate-180' : ''
                            }`}
                          />
                        </button>
                      </div>
                    </TableCell>
                    <TableCell className='text-right'>
                      <div className='flex items-center justify-end gap-1.5'>
                        {onEditRole && (
                          <Button
                            variant='ghost'
                            size='sm'
                            onClick={() => onEditRole(role)}
                            className='h-8 px-2 text-xs'
                            title={t('action-edit')}
                          >
                            <IconEdit className='h-3.5 w-3.5 mr-1' />
                            <span>{t('action-edit')}</span>
                          </Button>
                        )}
                        {onAssignRole && (
                          <Button
                            variant='outline'
                            size='sm'
                            onClick={() => onAssignRole(role.slug)}
                            className='h-8 px-2 text-xs'
                          >
                            {t('action-assign')}
                          </Button>
                        )}
                        {onDeleteRole && (
                          <Button
                            variant='ghost'
                            size='sm'
                            onClick={() => onDeleteRole(role)}
                            disabled={isSystem}
                            title={isSystem ? t('system-protected-tooltip') : t('action-delete')}
                            className='h-8 w-8 p-0 text-destructive hover:bg-destructive/10 disabled:opacity-30 disabled:hover:bg-transparent'
                          >
                            {isSystem ? (
                              <IconLock className='h-3.5 w-3.5 text-muted-foreground' />
                            ) : (
                              <IconTrash className='h-3.5 w-3.5' />
                            )}
                          </Button>
                        )}
                      </div>
                    </TableCell>
                  </TableRow>

                  {/* Expanded permissions drawer */}
                  {isExpanded && (
                    <TableRow className='bg-muted/20'>
                      <TableCell colSpan={6} className='p-4'>
                        <div className='space-y-2'>
                          <div className='flex items-center justify-between text-xs text-muted-foreground font-medium'>
                            <span>
                              {t('perms-granted-for', { name: role.displayName })}
                            </span>
                            <span>
                              {t('perms-total', { count: role.permissions.length })}
                            </span>
                          </div>
                          <div className='flex flex-wrap gap-1.5 max-h-48 overflow-y-auto p-2 bg-background rounded-lg border'>
                            {role.permissions.map((perm) => (
                              <Badge
                                key={perm}
                                variant='outline'
                                className='font-mono text-[11px] bg-muted/30'
                              >
                                {perm}
                              </Badge>
                            ))}
                          </div>
                        </div>
                      </TableCell>
                    </TableRow>
                  )}
                </React.Fragment>
              );
            })}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}
