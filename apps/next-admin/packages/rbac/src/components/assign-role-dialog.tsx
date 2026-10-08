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
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import { toast } from 'sonner';
import { useSession } from 'next-auth/react';
import { assignUserRole, type RoleInfo } from '../api/roles';

interface AssignRoleDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  roles: RoleInfo[];
  defaultRoleSlug?: string;
  defaultUserId?: string;
  token?: string | null;
  tenantSlug?: string | null;
  onSuccess?: () => void;
}

const UUID_REGEX =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export function AssignRoleDialog({
  open,
  onOpenChange,
  roles,
  defaultRoleSlug,
  defaultUserId = '',
  token,
  tenantSlug,
  onSuccess
}: AssignRoleDialogProps) {
  const t = useTranslations('rbac');
  const { data: session } = useSession();
  const effectiveToken = token ?? session?.user?.rustokToken ?? null;
  const effectiveTenantSlug = tenantSlug ?? session?.user?.tenantSlug ?? null;

  const [userId, setUserId] = React.useState(defaultUserId);
  const [roleSlug, setRoleSlug] = React.useState(
    defaultRoleSlug || roles[0]?.slug || 'customer'
  );
  const [isSubmitting, setIsSubmitting] = React.useState(false);

  React.useEffect(() => {
    if (defaultRoleSlug) {
      setRoleSlug(defaultRoleSlug);
    }
  }, [defaultRoleSlug]);

  React.useEffect(() => {
    if (defaultUserId) {
      setUserId(defaultUserId);
    }
  }, [defaultUserId]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmedId = userId.trim();
    if (!trimmedId) {
      toast.error(t('assign.err.user.id.required'));
      return;
    }
    if (!UUID_REGEX.test(trimmedId)) {
      toast.error(t('assign.err.uuid.invalid'));
      return;
    }
    if (!roleSlug) {
      toast.error(t('assign.err.role.required'));
      return;
    }

    setIsSubmitting(true);
    try {
      const result = await assignUserRole(
        { userId: trimmedId, role: roleSlug },
        { token: effectiveToken, tenantSlug: effectiveTenantSlug }
      );
      if (result.success) {
        toast.success(t('assign.toast.success'));
        onOpenChange(false);
        setUserId('');
        onSuccess?.();
      } else {
        toast.error(t('assign.toast.fail'));
      }
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : 'Unknown error';
      toast.error(t('assign.toast.error', { error: message }));
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className='sm:max-w-[440px]'>
        <form onSubmit={handleSubmit}>
          <DialogHeader>
            <DialogTitle>{t('assign.dialog.title')}</DialogTitle>
            <DialogDescription>{t('assign.dialog.desc')}</DialogDescription>
          </DialogHeader>

          <div className='grid gap-4 py-4'>
            <div className='grid gap-2'>
              <Label htmlFor='user-id'>{t('assign.field.user.id')}</Label>
              <Input
                id='user-id'
                placeholder={t('assign.field.user.id.placeholder')}
                value={userId}
                onChange={(e) => setUserId(e.target.value)}
                disabled={isSubmitting}
                className='font-mono text-sm'
              />
            </div>

            <div className='grid gap-2'>
              <Label htmlFor='role-select'>{t('assign.field.role')}</Label>
              <Select
                value={roleSlug}
                onValueChange={setRoleSlug}
                disabled={isSubmitting}
              >
                <SelectTrigger id='role-select'>
                  <SelectValue
                    placeholder={t('assign.field.role.placeholder')}
                  />
                </SelectTrigger>
                <SelectContent>
                  {roles.map((role) => (
                    <SelectItem key={role.slug} value={role.slug}>
                      <span className='font-medium'>{role.displayName}</span>
                      <span className='text-muted-foreground ml-2 font-mono text-xs'>
                        ({role.slug})
                      </span>
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>

          <DialogFooter>
            <Button
              type='button'
              variant='outline'
              onClick={() => onOpenChange(false)}
              disabled={isSubmitting}
            >
              {t('btn.cancel')}
            </Button>
            <Button type='submit' disabled={isSubmitting}>
              {isSubmitting
                ? t('assign.btn.submitting')
                : t('assign.btn.submit')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
