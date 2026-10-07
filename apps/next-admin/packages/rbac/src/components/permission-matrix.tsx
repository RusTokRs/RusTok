'use client';

import * as React from 'react';
import { useTranslations } from '@rustok/next-fluent';
import { Input } from '@/shared/ui/shadcn/input';
import { Button } from '@/shared/ui/shadcn/button';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Checkbox } from '@/shared/ui/shadcn/checkbox';
import {
  IconSearch,
  IconCheck,
  IconX,
  IconLock,
  IconShieldCheck,
  IconFolder
} from '@tabler/icons-react';
import type { PlatformPermissionItem } from '../api/roles';

interface PermissionMatrixProps {
  permissions: PlatformPermissionItem[];
  selectedPermissions: string[];
  onChange: (selected: string[]) => void;
  disabled?: boolean;
  isSuperAdmin?: boolean;
}

export function PermissionMatrix({
  permissions,
  selectedPermissions,
  onChange,
  disabled = false,
  isSuperAdmin = false
}: PermissionMatrixProps) {
  const t = useTranslations('rbac');
  const [search, setSearch] = React.useState('');

  // Group permissions by resource/module
  const grouped = React.useMemo(() => {
    const map = new Map<string, PlatformPermissionItem[]>();
    for (const item of permissions) {
      const list = map.get(item.resource) ?? [];
      list.push(item);
      map.set(item.resource, list);
    }
    return map;
  }, [permissions]);

  // Filtered groups based on search
  const filteredGroups = React.useMemo(() => {
    const q = search.trim().toLowerCase();
    const result = new Map<string, PlatformPermissionItem[]>();

    for (const [resource, items] of grouped.entries()) {
      if (!q) {
        result.set(resource, items);
        continue;
      }

      const matchingItems = items.filter(
        (item) =>
          item.resource.toLowerCase().includes(q) ||
          item.action.toLowerCase().includes(q) ||
          item.id.toLowerCase().includes(q) ||
          (item.description && item.description.toLowerCase().includes(q))
      );

      if (matchingItems.length > 0) {
        result.set(resource, matchingItems);
      }
    }

    return result;
  }, [grouped, search]);

  const selectedSet = React.useMemo(
    () => new Set(selectedPermissions),
    [selectedPermissions]
  );

  const togglePermission = (id: string) => {
    if (disabled || isSuperAdmin) return;
    const next = new Set(selectedSet);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    onChange(Array.from(next));
  };

  const selectAllInModule = (items: PlatformPermissionItem[]) => {
    if (disabled || isSuperAdmin) return;
    const next = new Set(selectedSet);
    for (const item of items) {
      next.add(item.id);
    }
    onChange(Array.from(next));
  };

  const deselectAllInModule = (items: PlatformPermissionItem[]) => {
    if (disabled || isSuperAdmin) return;
    const next = new Set(selectedSet);
    for (const item of items) {
      next.delete(item.id);
    }
    onChange(Array.from(next));
  };

  const selectAll = () => {
    if (disabled || isSuperAdmin) return;
    onChange(permissions.map((p) => p.id));
  };

  const clearAll = () => {
    if (disabled || isSuperAdmin) return;
    onChange([]);
  };

  return (
    <div className='space-y-4'>
      {/* Super Admin Notice */}
      {isSuperAdmin && (
        <div className='flex items-start gap-3 rounded-lg border border-amber-500/30 bg-amber-500/10 p-3.5 text-xs text-amber-900 dark:text-amber-200'>
          <IconLock className='h-4 w-4 shrink-0 text-amber-600 mt-0.5' />
          <div>
            <div className='font-semibold'>
              {t('superadmin-locked-notice')}
            </div>
          </div>
        </div>
      )}

      {/* Toolbar */}
      <div className='flex flex-col sm:flex-row sm:items-center justify-between gap-3'>
        <div className='relative flex-1 max-w-sm'>
          <IconSearch className='absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground' />
          <Input
            placeholder={t('matrix-search-placeholder')}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className='pl-9 h-9 text-xs'
          />
        </div>

        <div className='flex items-center gap-2'>
          <Badge variant='outline' className='font-mono text-xs py-1'>
            {t('matrix-selected-count', {
              selected: isSuperAdmin ? permissions.length : selectedPermissions.length,
              total: permissions.length
            })}
          </Badge>

          {!disabled && !isSuperAdmin && (
            <>
              <Button
                type='button'
                variant='outline'
                size='sm'
                onClick={selectAll}
                className='h-8 text-xs'
              >
                <IconCheck className='h-3.5 w-3.5 mr-1 text-emerald-600' />
                {t('matrix-select-all')}
              </Button>
              <Button
                type='button'
                variant='outline'
                size='sm'
                onClick={clearAll}
                className='h-8 text-xs'
              >
                <IconX className='h-3.5 w-3.5 mr-1 text-rose-600' />
                {t('matrix-clear-all')}
              </Button>
            </>
          )}
        </div>
      </div>

      {/* Modules List / Cards */}
      <div className='space-y-3 max-h-[480px] overflow-y-auto pr-1'>
        {Array.from(filteredGroups.entries()).map(([resource, items]) => {
          const moduleSelectedCount = items.filter((i) =>
            isSuperAdmin ? true : selectedSet.has(i.id)
          ).length;
          const allSelected = moduleSelectedCount === items.length;
          const noneSelected = moduleSelectedCount === 0;

          return (
            <div
              key={resource}
              className='rounded-lg border bg-card/60 p-3.5 shadow-sm space-y-3 transition-colors hover:border-primary/40'
            >
              {/* Module Header */}
              <div className='flex items-center justify-between gap-2 border-b pb-2.5'>
                <div className='flex items-center gap-2'>
                  <div className='p-1 rounded bg-primary/10 text-primary'>
                    <IconFolder className='h-4 w-4' />
                  </div>
                  <span className='font-semibold text-xs uppercase tracking-wider text-foreground'>
                    {resource.replace(/_/g, ' ')}
                  </span>
                  <Badge variant='secondary' className='text-[10px] font-mono'>
                    {moduleSelectedCount}/{items.length}
                  </Badge>
                </div>

                {!disabled && !isSuperAdmin && (
                  <div className='flex items-center gap-1.5'>
                    <Button
                      type='button'
                      variant='ghost'
                      size='sm'
                      onClick={() =>
                        allSelected
                          ? deselectAllInModule(items)
                          : selectAllInModule(items)
                      }
                      className='h-7 px-2 text-[11px] text-muted-foreground hover:text-foreground'
                    >
                      {allSelected
                        ? t('matrix-module-none')
                        : t('matrix-module-all')}
                    </Button>
                  </div>
                )}
              </div>

              {/* Permissions Checkbox Grid */}
              <div className='grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6 gap-2'>
                {items.map((item) => {
                  const isChecked = isSuperAdmin || selectedSet.has(item.id);
                  return (
                    <label
                      key={item.id}
                      className={`flex items-center gap-2 rounded-md border p-2 text-xs transition-colors cursor-pointer select-none ${
                        isChecked
                          ? 'border-primary/40 bg-primary/5 text-foreground font-medium'
                          : 'border-border/60 bg-background/50 text-muted-foreground hover:bg-muted/30'
                      } ${disabled || isSuperAdmin ? 'cursor-not-allowed opacity-80' : ''}`}
                    >
                      <Checkbox
                        checked={isChecked}
                        onCheckedChange={() => togglePermission(item.id)}
                        disabled={disabled || isSuperAdmin}
                        className='data-[state=checked]:bg-primary'
                      />
                      <span className='capitalize font-mono text-[11px] truncate' title={item.id}>
                        {item.action}
                      </span>
                    </label>
                  );
                })}
              </div>
            </div>
          );
        })}

        {filteredGroups.size === 0 && (
          <div className='rounded-lg border border-dashed p-8 text-center text-xs text-muted-foreground'>
            No permissions matching &ldquo;{search}&rdquo;
          </div>
        )}
      </div>
    </div>
  );
}
