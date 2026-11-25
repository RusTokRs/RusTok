'use client';

import * as React from 'react';
import { useTranslations } from '@rustok/next-fluent/client';
import {
  DataTableStatic,
  type DataTableStaticColumn
} from '@/widgets/data-table';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import { IconSearch, IconKey, IconFilter } from '@tabler/icons-react';
import type { PlatformPermissionItem, RoleInfo } from '../api/roles';

interface PermissionsTableProps {
  roles: RoleInfo[];
  platformPermissions?: PlatformPermissionItem[];
}

interface CatalogPermissionRow {
  slug: string;
  resource: string;
  action: string;
  module: string;
  moduleStatus: string;
  description?: string | null;
  roles: Array<{ slug: string; displayName: string }>;
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

function splitPermission(perm: string): { resource: string; action: string } {
  const sepIdx = perm.lastIndexOf(':');
  if (sepIdx !== -1) {
    return {
      resource: perm.slice(0, sepIdx),
      action: perm.slice(sepIdx + 1)
    };
  }
  const dotIdx = perm.lastIndexOf('.');
  if (dotIdx !== -1) {
    return {
      resource: perm.slice(0, dotIdx),
      action: perm.slice(dotIdx + 1)
    };
  }
  return { resource: 'core', action: perm };
}

export function PermissionsTable({
  roles,
  platformPermissions = []
}: PermissionsTableProps) {
  const t = useTranslations('rbac');
  const [search, setSearch] = React.useState('');
  const [selectedModule, setSelectedModule] = React.useState('all');

  const permissions = React.useMemo<CatalogPermissionRow[]>(() => {
    const map = new Map<string, CatalogPermissionRow>();

    for (const item of platformPermissions) {
      map.set(item.permission, {
        slug: item.permission,
        resource: item.resource,
        action: item.action,
        module: item.moduleSlug || item.resource,
        moduleStatus: item.moduleStatus || 'core',
        description: item.description,
        roles: []
      });
    }

    for (const role of roles) {
      for (const perm of role.permissions) {
        const existing = map.get(perm);
        const roleRef = { slug: role.slug, displayName: role.displayName };
        if (!existing) {
          const parsed = splitPermission(perm);
          map.set(perm, {
            slug: perm,
            resource: parsed.resource,
            action: parsed.action,
            module: parsed.resource,
            moduleStatus: 'core',
            roles: [roleRef]
          });
        } else if (!existing.roles.some((r) => r.slug === role.slug)) {
          existing.roles.push(roleRef);
        }
      }
    }

    return Array.from(map.values()).sort((a, b) =>
      a.slug.localeCompare(b.slug)
    );
  }, [roles, platformPermissions]);

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
        perm.resource.toLowerCase().includes(q) ||
        perm.module.toLowerCase().includes(q) ||
        perm.action.toLowerCase().includes(q) ||
        (perm.description?.toLowerCase().includes(q) ?? false) ||
        perm.roles.some(
          (r) =>
            r.displayName.toLowerCase().includes(q) ||
            r.slug.toLowerCase().includes(q)
        );

      return matchesModule && matchesSearch;
    });
  }, [permissions, selectedModule, search]);

  const columns: DataTableStaticColumn<CatalogPermissionRow>[] = [
    {
      id: 'module',
      header: t('perms-col-module'),
      headerClassName: 'w-[180px]',
      cell: (perm) => (
        <div className='flex items-center gap-1.5'>
          <Badge variant='outline' className='font-mono text-xs'>
            {perm.module}
          </Badge>
          {perm.moduleStatus === 'disabled' && (
            <Badge variant='secondary' className='text-[10px]'>
              {t('module-disabled')}
            </Badge>
          )}
        </div>
      )
    },
    {
      id: 'slug',
      header: t('perms-col-permission'),
      headerClassName: 'min-w-[240px]',
      cell: (perm) => (
        <div className='space-y-0.5'>
          <div className='flex items-center gap-2'>
            <IconKey className='text-muted-foreground h-3.5 w-3.5 flex-shrink-0' />
            <span className='text-foreground font-mono text-xs font-semibold'>
              {perm.slug}
            </span>
          </div>
          {perm.description && (
            <p className='text-muted-foreground text-xs'>{perm.description}</p>
          )}
        </div>
      )
    },
    {
      id: 'resource',
      header: t('perms-col-resource'),
      headerClassName: 'w-[160px]',
      cell: (perm) => (
        <div className='flex items-center gap-1.5'>
          <span className='font-mono text-xs'>{perm.resource}</span>
          <Badge variant='secondary' className='font-mono text-xs'>
            {perm.action}
          </Badge>
        </div>
      )
    },
    {
      id: 'roles',
      header: t('perms-col-roles'),
      cell: (perm) =>
        perm.roles.length > 0 ? (
          <div className='flex flex-wrap gap-1.5'>
            {perm.roles.map((roleRef) => (
              <Badge
                key={roleRef.slug}
                variant={ROLE_BADGE_VARIANT[roleRef.slug] ?? 'secondary'}
                className='text-xs'
              >
                {roleRef.displayName}
              </Badge>
            ))}
          </div>
        ) : (
          <span className='text-muted-foreground text-xs'>
            {t('perms-unassigned')}
          </span>
        )
    }
  ];

  const hasFilter = search.trim().length > 0 || selectedModule !== 'all';

  return (
    <div className='space-y-4'>
      {/* Filters row */}
      <div className='flex flex-col items-stretch justify-between gap-3 sm:flex-row sm:items-center'>
        <div className='relative max-w-sm flex-1'>
          <IconSearch className='text-muted-foreground absolute top-2.5 left-2.5 h-4 w-4' />
          <Input
            placeholder={t('perms-search-placeholder')}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className='pl-9'
          />
        </div>

        <div className='flex items-center gap-2'>
          <IconFilter className='text-muted-foreground h-4 w-4' />
          <Select value={selectedModule} onValueChange={setSelectedModule}>
            <SelectTrigger className='w-[200px]'>
              <SelectValue placeholder={t('perms-col-module')} />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value='all'>
                {t('perms-col-module')} ({modules.length})
              </SelectItem>
              {modules.map((mod) => (
                <SelectItem key={mod} value={mod}>
                  {mod}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          {hasFilter && (
            <Button
              type='button'
              variant='ghost'
              size='sm'
              onClick={() => {
                setSearch('');
                setSelectedModule('all');
              }}
            >
              {t('perms-clear-filter')}
            </Button>
          )}
        </div>
      </div>

      {/* Table view: markup lives in the shared primitive */}
      <DataTableStatic
        rows={filteredPermissions}
        columns={columns}
        getRowKey={(perm) => perm.slug}
        className='bg-card overflow-hidden rounded-xl shadow-sm'
        emptyState={t('perms-empty')}
      />

      <div className='text-muted-foreground flex items-center justify-between px-1 text-xs'>
        <span>
          {t('perms-showing-count', {
            shown: filteredPermissions.length,
            total: permissions.length
          })}
        </span>
      </div>
    </div>
  );
}
