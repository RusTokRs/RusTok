'use client';

import * as React from 'react';
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
  IconKey
} from '@tabler/icons-react';
import type { RoleInfo } from '../api/roles';

interface RolesTableProps {
  roles: RoleInfo[];
  onAssignRole?: (roleSlug: string) => void;
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

const ROLE_DESCRIPTION: Record<string, string> = {
  super_admin: 'Full unrestricted platform and tenant governance privileges',
  admin: 'Operational administrative control across modules and settings',
  manager: 'Catalog, content, orders, and fulfillment operations',
  customer: 'Standard storefront identity with read-only public access'
};

export function RolesTable({ roles, onAssignRole }: RolesTableProps) {
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

  const filteredRoles = React.useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return roles;
    return roles.filter(
      (role) =>
        role.displayName.toLowerCase().includes(q) ||
        role.slug.toLowerCase().includes(q) ||
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
              Platform Roles
            </CardTitle>
            <IconShieldLock className='h-4 w-4 text-primary' />
          </CardHeader>
          <CardContent>
            <div className='text-2xl font-bold'>{roles.length}</div>
            <p className='text-xs text-muted-foreground mt-1'>
              Active security archetypes
            </p>
          </CardContent>
        </Card>

        <Card className='shadow-none border'>
          <CardHeader className='flex flex-row items-center justify-between pb-2 space-y-0'>
            <CardTitle className='text-xs font-medium text-muted-foreground'>
              Unique Permissions
            </CardTitle>
            <IconKey className='h-4 w-4 text-emerald-600' />
          </CardHeader>
          <CardContent>
            <div className='text-2xl font-bold'>{totalUniquePermissions}</div>
            <p className='text-xs text-muted-foreground mt-1'>
              Catalogued across all modules
            </p>
          </CardContent>
        </Card>

        <Card className='shadow-none border'>
          <CardHeader className='flex flex-row items-center justify-between pb-2 space-y-0'>
            <CardTitle className='text-xs font-medium text-muted-foreground'>
              Administrative Roles
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
              Super Admin & Admin tiers
            </p>
          </CardContent>
        </Card>

        <Card className='shadow-none border'>
          <CardHeader className='flex flex-row items-center justify-between pb-2 space-y-0'>
            <CardTitle className='text-xs font-medium text-muted-foreground'>
              Standard Roles
            </CardTitle>
            <IconUsers className='h-4 w-4 text-sky-600' />
          </CardHeader>
          <CardContent>
            <div className='text-2xl font-bold'>
              {
                roles.filter(
                  (r) => r.slug !== 'super_admin' && r.slug !== 'admin'
                ).length
              }
            </div>
            <p className='text-xs text-muted-foreground mt-1'>
              Staff & Customer tiers
            </p>
          </CardContent>
        </Card>
      </div>

      {/* Search Bar */}
      <div className='flex items-center justify-between gap-4'>
        <div className='relative flex-1 max-w-sm'>
          <IconSearch className='absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground' />
          <Input
            placeholder='Search roles or permissions...'
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className='pl-9'
          />
        </div>
      </div>

      {/* Main Table */}
      <div className='rounded-xl border bg-card shadow-sm overflow-hidden'>
        <Table>
          <TableHeader>
            <TableRow className='bg-muted/50'>
              <TableHead className='w-[220px]'>Role</TableHead>
              <TableHead className='w-[130px]'>Slug</TableHead>
              <TableHead className='w-[120px]'>Permissions</TableHead>
              <TableHead className='min-w-[300px]'>Description & Preview</TableHead>
              <TableHead className='w-[150px] text-right'>Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {filteredRoles.map((role) => {
              const isExpanded = expandedRole === role.slug;
              const description =
                ROLE_DESCRIPTION[role.slug] || 'Custom platform role';

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
                      <Badge variant='secondary' className='font-mono text-xs'>
                        {role.permissions.length} perms
                      </Badge>
                    </TableCell>
                    <TableCell>
                      <div className='space-y-1.5'>
                        <p className='text-xs text-muted-foreground'>
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
                            ? 'Hide permissions'
                            : `View ${role.permissions.length} permissions`}
                          <IconChevronDown
                            className={`h-3 w-3 transition-transform ${
                              isExpanded ? 'rotate-180' : ''
                            }`}
                          />
                        </button>
                      </div>
                    </TableCell>
                    <TableCell className='text-right'>
                      <Button
                        variant='outline'
                        size='sm'
                        onClick={() => onAssignRole?.(role.slug)}
                        className='text-xs'
                      >
                        Assign to User
                      </Button>
                    </TableCell>
                  </TableRow>

                  {/* Expanded permissions drawer */}
                  {isExpanded && (
                    <TableRow className='bg-muted/20'>
                      <TableCell colSpan={5} className='p-4'>
                        <div className='space-y-2'>
                          <div className='flex items-center justify-between text-xs text-muted-foreground font-medium'>
                            <span>Granted Permissions for {role.displayName}:</span>
                            <span>{role.permissions.length} total</span>
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
