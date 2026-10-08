'use client';

import * as React from 'react';
import {
  DataTableStatic,
  type DataTableStaticColumn
} from '@/widgets/data-table';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Input } from '@/shared/ui/shadcn/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import { IconSearch, IconKey, IconFilter } from '@tabler/icons-react';
import type { RoleInfo } from '../api/roles';
import type { PermissionRecord } from '../types';

interface PermissionsTableProps {
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

export function PermissionsTable({ roles }: PermissionsTableProps) {
  const [search, setSearch] = React.useState('');
  const [selectedModule, setSelectedModule] = React.useState('all');

  // Extract all distinct permissions from all roles
  const permissions = React.useMemo<PermissionRecord[]>(() => {
    const map = new Map<string, PermissionRecord>();

    for (const role of roles) {
      for (const perm of role.permissions) {
        if (!map.has(perm)) {
          const sepIdx =
            perm.indexOf(':') !== -1 ? perm.indexOf(':') : perm.indexOf('.');
          const mod = sepIdx !== -1 ? perm.slice(0, sepIdx) : 'core';
          const act = sepIdx !== -1 ? perm.slice(sepIdx + 1) : perm;
          map.set(perm, {
            slug: perm,
            module: mod,
            action: act,
            roles: [role.displayName]
          });
        } else {
          const existing = map.get(perm)!;
          if (!existing.roles.includes(role.displayName)) {
            existing.roles.push(role.displayName);
          }
        }
      }
    }

    return Array.from(map.values()).sort((a, b) =>
      a.slug.localeCompare(b.slug)
    );
  }, [roles]);

  const modules = React.useMemo(() => {
    const mods = new Set(permissions.map((p) => p.module));
    return Array.from(mods).sort();
  }, [permissions]);

  const filteredPermissions = React.useMemo(() => {
    return permissions.filter((perm) => {
      const matchesModule =
        selectedModule === 'all' || perm.module === selectedModule;
      const q = search.trim().toLowerCase();
      const matchesSearch =
        !q ||
        perm.slug.toLowerCase().includes(q) ||
        perm.module.toLowerCase().includes(q) ||
        perm.action.toLowerCase().includes(q) ||
        perm.roles.some((r) => r.toLowerCase().includes(q));

      return matchesModule && matchesSearch;
    });
  }, [permissions, selectedModule, search]);

  // The shared static table owns the header, the row loop and the empty state; this component only
  // declares what a permission column shows.
  const columns: DataTableStaticColumn<PermissionRecord>[] = [
    {
      id: 'module',
      header: 'Module',
      headerClassName: 'w-[140px]',
      cell: (perm) => (
        <Badge variant='outline' className='font-mono text-xs capitalize'>
          {perm.module}
        </Badge>
      )
    },
    {
      id: 'slug',
      header: 'Permission Key',
      headerClassName: 'min-w-[220px]',
      cell: (perm) => (
        <div className='flex items-center gap-2'>
          <IconKey className='text-muted-foreground h-3.5 w-3.5 flex-shrink-0' />
          <span className='text-foreground font-mono text-xs font-semibold'>
            {perm.slug}
          </span>
        </div>
      )
    },
    {
      id: 'action',
      header: 'Action',
      headerClassName: 'w-[120px]',
      cell: (perm) => (
        <Badge variant='secondary' className='font-mono text-xs'>
          {perm.action}
        </Badge>
      )
    },
    {
      id: 'roles',
      header: 'Granted to Roles',
      cell: (perm) => (
        <div className='flex flex-wrap gap-1.5'>
          {perm.roles.map((roleName) => (
            <Badge
              key={roleName}
              variant={
                roleName.toLowerCase().includes('admin')
                  ? 'default'
                  : 'secondary'
              }
              className='text-xs'
            >
              {roleName}
            </Badge>
          ))}
        </div>
      )
    }
  ];

  return (
    <div className='space-y-4'>
      {/* Filters row */}
      <div className='flex flex-col items-stretch justify-between gap-3 sm:flex-row sm:items-center'>
        <div className='relative max-w-sm flex-1'>
          <IconSearch className='text-muted-foreground absolute top-2.5 left-2.5 h-4 w-4' />
          <Input
            placeholder='Search permissions or roles...'
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className='pl-9'
          />
        </div>

        <div className='flex items-center gap-2'>
          <IconFilter className='text-muted-foreground h-4 w-4' />
          <Select value={selectedModule} onValueChange={setSelectedModule}>
            <SelectTrigger className='w-[180px]'>
              <SelectValue placeholder='All modules' />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value='all'>
                All Modules ({modules.length})
              </SelectItem>
              {modules.map((mod) => (
                <SelectItem key={mod} value={mod}>
                  {mod}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>

      {/* Table view: markup lives in the shared primitive */}
      <DataTableStatic
        rows={filteredPermissions}
        columns={columns}
        getRowKey={(perm) => perm.slug}
        className='bg-card overflow-hidden rounded-xl shadow-sm'
        emptyState='No permissions found matching the filter.'
      />

      <div className='text-muted-foreground flex items-center justify-between px-1 text-xs'>
        <span>
          Showing {filteredPermissions.length} of {permissions.length} total
          permissions
        </span>
        <span>{modules.length} active modules</span>
      </div>
    </div>
  );
}
