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
          <IconLock className='mt-0.5 h-4 w-4 shrink-0 text-amber-600' />
          <div>
            <div className='font-semibold'>{t('superadmin-locked-notice')}</div>
          </div>
        </div>
      )}

      {/* Toolbar */}
      <div className='flex flex-col justify-between gap-3 sm:flex-row sm:items-center'>
        <div className='relative max-w-sm flex-1'>
          <IconSearch className='text-muted-foreground absolute top-2.5 left-2.5 h-4 w-4' />
          <Input
            placeholder={t('matrix-search-placeholder')}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className='h-9 pl-9 text-xs'
          />
        </div>

        <div className='flex items-center gap-2'>
          <Badge variant='outline' className='py-1 font-mono text-xs'>
            {t('matrix-selected-count', {
              selected: isSuperAdmin
                ? permissions.length
                : selectedPermissions.length,
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
                <IconCheck className='mr-1 h-3.5 w-3.5 text-emerald-600' />
                {t('matrix-select-all')}
              </Button>
              <Button
                type='button'
                variant='outline'
                size='sm'
                onClick={clearAll}
                className='h-8 text-xs'
              >
                <IconX className='mr-1 h-3.5 w-3.5 text-rose-600' />
                {t('matrix-clear-all')}
              </Button>
            </>
          )}
        </div>
      </div>

      {/* Modules List / Cards */}
      <div className='max-h-[480px] space-y-3 overflow-y-auto pr-1'>
        {Array.from(filteredGroups.entries()).map(([resource, items]) => {
          const moduleSelectedCount = items.filter((i) =>
            isSuperAdmin ? true : selectedSet.has(i.id)
          ).length;
          const allSelected = moduleSelectedCount === items.length;
          const noneSelected = moduleSelectedCount === 0;

          return (
            <div
              key={resource}
              className='bg-card/60 hover:border-primary/40 space-y-3 rounded-lg border p-3.5 shadow-sm transition-colors'
            >
              {/* Module Header */}
              <div className='flex items-center justify-between gap-2 border-b pb-2.5'>
                <div className='flex items-center gap-2'>
                  <div className='bg-primary/10 text-primary rounded p-1'>
                    <IconFolder className='h-4 w-4' />
                  </div>
                  <span className='text-foreground text-xs font-semibold tracking-wider uppercase'>
                    {resource.replace(/_/g, ' ')}
                  </span>
                  <Badge variant='secondary' className='font-mono text-[10px]'>
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
                      className='text-muted-foreground hover:text-foreground h-7 px-2 text-[11px]'
                    >
                      {allSelected
                        ? t('matrix-module-none')
                        : t('matrix-module-all')}
                    </Button>
                  </div>
                )}
              </div>

              {/* Permissions Checkbox Grid */}
              <div className='grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6'>
                {items.map((item) => {
                  const isChecked = isSuperAdmin || selectedSet.has(item.id);
                  return (
                    <label
                      key={item.id}
                      className={`flex cursor-pointer items-center gap-2 rounded-md border p-2 text-xs transition-colors select-none ${
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
                      <span
                        className='truncate font-mono text-[11px] capitalize'
                        title={item.id}
                      >
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
          <div className='text-muted-foreground rounded-lg border border-dashed p-8 text-center text-xs'>
            No permissions matching &ldquo;{search}&rdquo;
          </div>
        )}
      </div>
    </div>
  );
}
