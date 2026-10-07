'use client';

import * as React from 'react';
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
      toast.error('User ID is required');
      return;
    }
    if (!UUID_REGEX.test(trimmedId)) {
      toast.error(
        'User ID must be a valid UUID (e.g. 00000000-0000-0000-0000-000000000000)'
      );
      return;
    }
    if (!roleSlug) {
      toast.error('Please select a role');
      return;
    }

    setIsSubmitting(true);
    try {
      const result = await assignUserRole(
        { userId: trimmedId, role: roleSlug },
        { token: effectiveToken, tenantSlug: effectiveTenantSlug }
      );
      if (result.success) {
        toast.success(`Role assigned successfully to user`);
        onOpenChange(false);
        setUserId('');
        onSuccess?.();
      } else {
        toast.error('Failed to assign role');
      }
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : 'Unknown error';
      toast.error(`Assignment error: ${message}`);
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className='sm:max-w-[440px]'>
        <form onSubmit={handleSubmit}>
          <DialogHeader>
            <DialogTitle>Assign Role to User</DialogTitle>
            <DialogDescription>
              Assign or update the permission role for a specific user ID.
            </DialogDescription>
          </DialogHeader>

          <div className='grid gap-4 py-4'>
            <div className='grid gap-2'>
              <Label htmlFor='user-id'>User ID (UUID)</Label>
              <Input
                id='user-id'
                placeholder='e.g. 00000000-0000-0000-0000-000000000000'
                value={userId}
                onChange={(e) => setUserId(e.target.value)}
                disabled={isSubmitting}
                className='font-mono text-sm'
              />
            </div>

            <div className='grid gap-2'>
              <Label htmlFor='role-select'>Role</Label>
              <Select
                value={roleSlug}
                onValueChange={setRoleSlug}
                disabled={isSubmitting}
              >
                <SelectTrigger id='role-select'>
                  <SelectValue placeholder='Select a role' />
                </SelectTrigger>
                <SelectContent>
                  {roles.map((role) => (
                    <SelectItem key={role.slug} value={role.slug}>
                      <span className='font-medium'>{role.displayName}</span>
                      <span className='ml-2 text-xs text-muted-foreground font-mono'>
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
              Cancel
            </Button>
            <Button type='submit' disabled={isSubmitting}>
              {isSubmitting ? 'Assigning...' : 'Assign Role'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
