'use client';

import * as React from 'react';
import { useTranslations } from '@rustok/next-fluent';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from '@/shared/ui/shadcn/dialog';
import { Input } from '@/shared/ui/shadcn/input';
import { Button } from '@/shared/ui/shadcn/button';
import { toast } from 'sonner';
import { createRole, type PlatformPermissionItem, type RoleInfo } from '../api/roles';
import { PermissionMatrix } from './permission-matrix';

interface CreateRoleDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  platformPermissions: PlatformPermissionItem[];
  onCreated?: (role: RoleInfo) => void;
  token?: string | null;
  tenantSlug?: string | null;
}

export function CreateRoleDialog({
  open,
  onOpenChange,
  platformPermissions,
  onCreated,
  token,
  tenantSlug
}: CreateRoleDialogProps) {
  const t = useTranslations('rbac');
  const [name, setName] = React.useState('');
  const [slug, setSlug] = React.useState('');
  const [slugManuallyEdited, setSlugManuallyEdited] = React.useState(false);
  const [description, setDescription] = React.useState('');
  const [selectedPermissions, setSelectedPermissions] = React.useState<string[]>([]);
  const [submitting, setSubmitting] = React.useState(false);

  // Auto-slug from name
  const handleNameChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const nextName = e.target.value;
    setName(nextName);
    if (!slugManuallyEdited) {
      const derived = nextName
        .toLowerCase()
        .trim()
        .replace(/\s+/g, '_')
        .replace(/[^a-z0-9_-]/g, '');
      setSlug(derived);
    }
  };

  const handleSlugChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setSlugManuallyEdited(true);
    setSlug(
      e.target.value
        .toLowerCase()
        .replace(/\s+/g, '_')
        .replace(/[^a-z0-9_-]/g, '')
    );
  };

  const resetForm = () => {
    setName('');
    setSlug('');
    setSlugManuallyEdited(false);
    setDescription('');
    setSelectedPermissions([]);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const cleanName = name.trim();
    const cleanSlug = slug.trim().toLowerCase();

    if (!cleanName) {
      toast.error('Role name is required');
      return;
    }
    if (cleanSlug.length < 2) {
      toast.error('Role slug must be at least 2 characters');
      return;
    }

    const reserved = ['super_admin', 'admin', 'manager', 'customer'];
    if (reserved.includes(cleanSlug)) {
      toast.error(`'${cleanSlug}' is a reserved platform built-in role slug`);
      return;
    }

    setSubmitting(true);
    try {
      const result = await createRole(
        {
          name: cleanName,
          slug: cleanSlug,
          description: description.trim() || undefined,
          permissions: selectedPermissions
        },
        { token, tenantSlug }
      );

      if (result.success && result.role) {
        toast.success(t('toast-create-success'));
        onCreated?.(result.role);
        resetForm();
        onOpenChange(false);
      } else {
        toast.error(t('toast-error'));
      }
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : t('toast-error');
      toast.error(message);
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) resetForm();
        onOpenChange(next);
      }}
    >
      <DialogContent className='max-w-4xl max-h-[90vh] flex flex-col p-6 overflow-hidden'>
        <DialogHeader>
          <DialogTitle>{t('create-dialog-title')}</DialogTitle>
          <DialogDescription>{t('create-dialog-desc')}</DialogDescription>
        </DialogHeader>

        <form onSubmit={handleSubmit} className='flex-1 flex flex-col min-h-0 space-y-4'>
          {/* Metadata Fields */}
          <div className='grid grid-cols-1 sm:grid-cols-2 gap-4 pt-2'>
            <div className='space-y-1.5'>
              <label className='text-xs font-medium text-foreground'>
                {t('field-name')} *
              </label>
              <Input
                placeholder={t('field-name-placeholder')}
                value={name}
                onChange={handleNameChange}
                required
                className='h-9 text-xs'
              />
            </div>

            <div className='space-y-1.5'>
              <label className='text-xs font-medium text-foreground'>
                {t('field-slug')} *
              </label>
              <Input
                placeholder={t('field-slug-placeholder')}
                value={slug}
                onChange={handleSlugChange}
                required
                className='h-9 text-xs font-mono'
              />
              <p className='text-[10px] text-muted-foreground'>
                {t('field-slug-help')}
              </p>
            </div>

            <div className='sm:col-span-2 space-y-1.5'>
              <label className='text-xs font-medium text-foreground'>
                {t('field-description')}
              </label>
              <Input
                placeholder={t('field-description-placeholder')}
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                className='h-9 text-xs'
              />
            </div>
          </div>

          {/* Matrix Section */}
          <div className='flex-1 min-h-0 border-t pt-3'>
            <div className='mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground'>
              {t('matrix-title')}
            </div>
            <PermissionMatrix
              permissions={platformPermissions}
              selectedPermissions={selectedPermissions}
              onChange={setSelectedPermissions}
            />
          </div>

          <DialogFooter className='border-t pt-3'>
            <Button
              type='button'
              variant='outline'
              onClick={() => onOpenChange(false)}
              disabled={submitting}
              className='text-xs'
            >
              {t('btn-cancel')}
            </Button>
            <Button type='submit' disabled={submitting} className='text-xs'>
              {submitting ? t('btn-saving') : t('btn-create')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
