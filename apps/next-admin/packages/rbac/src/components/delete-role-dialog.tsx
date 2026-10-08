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
import { Button } from '@/shared/ui/shadcn/button';
import { IconAlertTriangle, IconLock } from '@tabler/icons-react';
import { toast } from 'sonner';
import { deleteRole, type RoleInfo } from '../api/roles';

interface DeleteRoleDialogProps {
  role: RoleInfo | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onDeleted?: (slug: string) => void;
  token?: string | null;
  tenantSlug?: string | null;
}

export function DeleteRoleDialog({
  role,
  open,
  onOpenChange,
  onDeleted,
  token,
  tenantSlug
}: DeleteRoleDialogProps) {
  const t = useTranslations('rbac');
  const [submitting, setSubmitting] = React.useState(false);

  if (!role) return null;

  const isSystem =
    Boolean(role.isSystem) ||
    ['super_admin', 'admin', 'manager', 'customer'].includes(role.slug);

  const handleDelete = async () => {
    if (isSystem) {
      toast.error(t('system-protected-tooltip'));
      return;
    }

    setSubmitting(true);
    try {
      const result = await deleteRole(role.slug, { token, tenantSlug });
      if (result.success) {
        toast.success(t('toast-delete-success'));
        onDeleted?.(role.slug);
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
      <DialogContent className='max-w-md p-6'>
        <DialogHeader>
          <div className='flex items-center gap-3'>
            <div
              className={`rounded-full p-2 ${isSystem ? 'bg-amber-500/10 text-amber-600' : 'bg-destructive/10 text-destructive'}`}
            >
              {isSystem ? (
                <IconLock className='h-5 w-5' />
              ) : (
                <IconAlertTriangle className='h-5 w-5' />
              )}
            </div>
            <div>
              <DialogTitle>
                {t('delete-dialog-title', { name: role.displayName })}
              </DialogTitle>
              <DialogDescription className='mt-1'>
                {t('delete-dialog-desc')}
              </DialogDescription>
            </div>
          </div>
        </DialogHeader>

        {isSystem ? (
          <div className='mt-2 rounded-lg border border-amber-500/30 bg-amber-500/10 p-3 text-xs text-amber-900 dark:text-amber-200'>
            {t('system-protected-tooltip')}
          </div>
        ) : (
          <div className='border-border bg-muted/40 text-muted-foreground mt-2 rounded-lg border p-3 text-xs'>
            {t('delete-dialog-warning')}
          </div>
        )}

        <DialogFooter className='mt-4 flex gap-2 sm:justify-end'>
          <Button
            type='button'
            variant='outline'
            onClick={() => onOpenChange(false)}
            disabled={submitting}
            className='text-xs'
          >
            {t('btn-cancel')}
          </Button>
          <Button
            type='button'
            variant='destructive'
            onClick={handleDelete}
            disabled={submitting || isSystem}
            className='text-xs'
          >
            {submitting ? t('btn-deleting') : t('btn-delete')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
