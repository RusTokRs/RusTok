'use client';

import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import { graphqlRequest } from '@/shared/api/graphql';
import { useSession } from 'next-auth/react';
import Link from 'next/link';
import { useEffect, useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from '@rustok/next-fluent';
import { listRoles, type RoleInfo } from '@rustok/rbac-admin';

interface UserDetail {
  id: string;
  email: string;
  name: string | null;
  role: string;
  status: string;
  createdAt: string;
  tenantName: string | null;
}

const USER_QUERY = `query User($id: UUID!) { user(id: $id) { id email name role status createdAt tenantName } }`;

const UPDATE_USER_MUTATION = `
mutation UpdateUser($id: UUID!, $input: UpdateUserInput!) {
  updateUser(id: $id, input: $input) {
    id email name role status createdAt tenantName
  }
}`;

const FALLBACK_ROLES: RoleInfo[] = [
  { slug: 'super_admin', displayName: 'Super Admin', permissions: [], isSystem: true },
  { slug: 'admin', displayName: 'Admin', permissions: [], isSystem: true },
  { slug: 'manager', displayName: 'Manager', permissions: [], isSystem: true },
  { slug: 'customer', displayName: 'Customer', permissions: [], isSystem: true }
];

export default function UserDetailView({ userId }: { userId: string }) {
  const t = useTranslations('users');
  const { data: session } = useSession();
  const token = session?.user?.rustokToken;
  const tenantSlug = session?.user?.tenantSlug;

  const [user, setUser] = useState<UserDetail | null>(null);
  const [roles, setRoles] = useState<RoleInfo[]>(FALLBACK_ROLES);
  const [isLoading, setIsLoading] = useState(true);
  const [isEditing, setIsEditing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [editName, setEditName] = useState('');
  const [editRole, setEditRole] = useState('');

  useEffect(() => {
    if (!token) return;
    (async () => {
      try {
        const [userData, fetchedRoles] = await Promise.all([
          graphqlRequest<{ id: string }, { user: UserDetail | null }>(
            USER_QUERY,
            { id: userId },
            token,
            tenantSlug
          ),
          listRoles({ token, tenantSlug }).catch(() => FALLBACK_ROLES)
        ]);

        setUser(userData.user);
        if (fetchedRoles && fetchedRoles.length > 0) {
          setRoles(fetchedRoles);
        }
        if (userData.user) {
          setEditName(userData.user.name ?? '');
          setEditRole(userData.user.role?.toLowerCase() ?? 'customer');
        }
      } catch {
        toast.error(t('toast.detail.load.error'));
      } finally {
        setIsLoading(false);
      }
    })();
  }, [userId, token, tenantSlug, t]);

  const handleSave = async () => {
    if (!token || !user) return;
    setIsSaving(true);
    try {
      const data = await graphqlRequest<object, { updateUser: UserDetail }>(
        UPDATE_USER_MUTATION,
        {
          id: userId,
          input: { name: editName.trim() || null, role: editRole || undefined }
        },
        token,
        tenantSlug
      );
      setUser(data.updateUser);
      setIsEditing(false);
      toast.success(t('toast.detail.update.success'));
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : t('toast.detail.update.error')
      );
    } finally {
      setIsSaving(false);
    }
  };

  const handleDisable = async () => {
    if (!token || !user) return;
    try {
      const data = await graphqlRequest<
        { id: string; input: { status: string } },
        { updateUser: UserDetail }
      >(
        UPDATE_USER_MUTATION,
        { id: userId, input: { status: 'INACTIVE' } },
        token,
        tenantSlug
      );
      setUser(data.updateUser);
      toast.success(t('toast.detail.deactivate.success'));
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : t('toast.detail.deactivate.error')
      );
    }
  };

  if (isLoading)
    return <p className='text-muted-foreground text-sm'>{t('detail.loading')}</p>;
  if (!user) return <p className='text-sm text-red-600'>{t('detail.empty')}</p>;

  return (
    <div className='space-y-4'>
      <div className='flex items-center gap-2'>
        <Button variant='outline' size='sm' asChild>
          <Link href='/dashboard/users'>← {t('detail.back')}</Link>
        </Button>
        {!isEditing && (
          <>
            <Button size='sm' onClick={() => setIsEditing(true)}>
              {t('detail.edit')}
            </Button>
            {user.status !== 'INACTIVE' && (
              <Button size='sm' variant='destructive' onClick={handleDisable}>
                {t('detail.deactivate')}
              </Button>
            )}
          </>
        )}
        {isEditing && (
          <>
            <Button size='sm' onClick={handleSave} disabled={isSaving}>
              {isSaving ? t('detail.saving') : t('detail.save')}
            </Button>
            <Button
              size='sm'
              variant='outline'
              onClick={() => setIsEditing(false)}
            >
              {t('detail.cancel')}
            </Button>
          </>
        )}
      </div>

      <Card>
        <CardHeader>
          <CardTitle>{user.name || user.email}</CardTitle>
        </CardHeader>
        <CardContent className='space-y-4'>
          {isEditing ? (
            <div className='grid gap-4 md:grid-cols-2'>
              <div>
                <Label htmlFor='edit-name'>{t('detail.name')}</Label>
                <Input
                  id='edit-name'
                  value={editName}
                  onChange={(e) => setEditName(e.target.value)}
                  placeholder={t('detail.name.placeholder')}
                />
              </div>
              <div>
                <Label htmlFor='edit-role'>{t('detail.role')}</Label>
                <select
                  id='edit-role'
                  value={editRole}
                  onChange={(e) => setEditRole(e.target.value)}
                  className='border-input bg-background ring-offset-background placeholder:text-muted-foreground focus-visible:ring-ring flex h-10 w-full rounded-md border px-3 py-2 text-sm focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none'
                >
                  {roles.map((r) => (
                    <option key={r.slug} value={r.slug}>
                      {r.displayName} ({r.slug})
                    </option>
                  ))}
                </select>
              </div>
            </div>
          ) : (
            <div className='grid gap-3 md:grid-cols-2 lg:grid-cols-3'>
              {[
                { label: t('detail.email'), value: user.email },
                { label: t('detail.name'), value: user.name || '—' },
                { label: t('detail.role'), value: user.role },
                {
                  label: t('detail.status'),
                  value: (
                    <Badge
                      variant={
                        user.status === 'ACTIVE' ? 'default' : 'secondary'
                      }
                    >
                      {user.status}
                    </Badge>
                  )
                },
                { label: t('detail.workspace'), value: user.tenantName || '—' },
                {
                  label: t('detail.member.since'),
                  value: new Date(user.createdAt).toLocaleDateString()
                },
                {
                  label: t('detail.id'),
                  value: <span className='font-mono text-xs'>{user.id}</span>
                }
              ].map(({ label, value }) => (
                <div key={label}>
                  <p className='text-muted-foreground text-xs font-medium tracking-wider uppercase'>
                    {label}
                  </p>
                  <div className='mt-1 text-sm font-medium'>{value}</div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

