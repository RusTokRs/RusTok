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
import { Badge } from '@/shared/ui/shadcn/badge';
import { toast } from 'sonner';
import { updateRole, type PlatformPermissionItem, type RoleInfo } from '../api/roles';
import { PermissionMatrix } from './permission-matrix';

interface EditRoleDialogProps {
  role: RoleInfo | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  platformPermissions: PlatformPermissionItem[];
  onUpdated?: (role: RoleInfo) => void;
  token?: string | null;
  tenantSlug?: string | null;
}

export function EditRoleDialog({
  role,
  open,
  onOpenChange,
  platformPermissions,
  onUpdated,
  token,
  tenantSlug
}: EditRoleDialogProps) {
  const t = useTranslations('rbac');
  const [name, setName] = React.useState('');
  const [description, setDescription] = React.useState('');
  const [selectedPermissions, setSelectedPermissions] = React.useState<string[]>([]);
  const [submitting, setSubmitting] = React.useState(false);

  React.useEffect(() => {
    if (role) {
      setName(role.displayName);
      setDescription(role.description || '');
      setSelectedPermissions([...role.permissions]);
    }
  }, [role]);

  if (!role) return null;

  const isSuperAdmin = role.slug === 'super_admin';
  const isSystem = Boolean(role.isSystem);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const cleanName = name.trim();

    if (!cleanName) {
      toast.error('Role name is required');
      return;
    }

    setSubmitting(true);
    try {
      const result = await updateRole(
        {
          slug: role.slug,
          name: cleanName,
          description: description.trim() || undefined,
          permissions: isSuperAdmin ? undefined : selectedPermissions
        },
        { token, tenantSlug }
      );

      if (result.success && result.role) {
        toast.success(t('toast-update-success'));
        onUpdated?.(result.role);
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
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className='max-w-4xl max-h-[90vh] flex flex-col p-6 overflow-hidden'>
        <DialogHeader>
          <div className='flex items-center gap-2'>
            <DialogTitle>{t('edit-dialog-title', { name: role.displayName })}</DialogTitle>
            <Badge variant={isSystem ? 'secondary' : 'outline'} className='text-xs'>
              {isSystem ? t('badge-system') : t('badge-custom')}
            </Badge>
          </div>
          <DialogDescription>{t('edit-dialog-desc')}</DialogDescription>
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
                onChange={(e) => setName(e.target.value)}
                required
                className='h-9 text-xs'
              />
            </div>

            <div className='space-y-1.5'>
              <label className='text-xs font-medium text-foreground'>
                {t('field-slug')}
              </label>
              <Input
                value={role.slug}
                disabled
                className='h-9 text-xs font-mono bg-muted/50 cursor-not-allowed'
              />
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
              isSuperAdmin={isSuperAdmin}
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
              {submitting ? t('btn-saving') : t('btn-save')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
