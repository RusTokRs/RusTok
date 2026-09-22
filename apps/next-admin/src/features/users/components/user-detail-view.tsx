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

const DISABLE_USER_MUTATION = `
mutation DisableUser($id: UUID!) {
  disableUser(id: $id) {
    id email name role status createdAt tenantName
  }
}`;

export default function UserDetailView({ userId }: { userId: string }) {
  const { data: session } = useSession();
  const token = session?.user?.rustokToken;
  const tenantSlug = session?.user?.tenantSlug;

  const [user, setUser] = useState<UserDetail | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isEditing, setIsEditing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [editName, setEditName] = useState('');
  const [editRole, setEditRole] = useState('');

  useEffect(() => {
    if (!token) return;
    (async () => {
      try {
        const data = await graphqlRequest<
          { id: string },
          { user: UserDetail | null }
        >(USER_QUERY, { id: userId }, token, tenantSlug);
        setUser(data.user);
        if (data.user) {
          setEditName(data.user.name ?? '');
          setEditRole(data.user.role);
        }
      } catch {
        toast.error('Failed to load user');
      } finally {
        setIsLoading(false);
      }
    })();
  }, [userId, token, tenantSlug]);

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
      toast.success('User updated');
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to update user');
    } finally {
      setIsSaving(false);
    }
  };

  const handleDisable = async () => {
    if (!token || !user) return;
    try {
      const data = await graphqlRequest<
        { id: string },
        { disableUser: UserDetail }
      >(DISABLE_USER_MUTATION, { id: userId }, token, tenantSlug);
      setUser(data.disableUser);
      toast.success('User deactivated');
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : 'Failed to disable user'
      );
    }
  };

  if (isLoading)
    return <p className='text-muted-foreground text-sm'>Loading...</p>;
  if (!user) return <p className='text-sm text-red-600'>User not found.</p>;

  return (
    <div className='space-y-4'>
      <div className='flex items-center gap-2'>
        <Button variant='outline' size='sm' asChild>
          <Link href='/dashboard/users'>← Back to Users</Link>
        </Button>
        {!isEditing && (
          <>
            <Button size='sm' onClick={() => setIsEditing(true)}>
              Edit
            </Button>
            {user.status !== 'INACTIVE' && (
              <Button size='sm' variant='destructive' onClick={handleDisable}>
                Deactivate
              </Button>
            )}
          </>
        )}
        {isEditing && (
          <>
            <Button size='sm' onClick={handleSave} disabled={isSaving}>
              {isSaving ? 'Saving...' : 'Save'}
            </Button>
            <Button
              size='sm'
              variant='outline'
              onClick={() => setIsEditing(false)}
            >
              Cancel
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
                <Label htmlFor='edit-name'>Name</Label>
                <Input
                  id='edit-name'
                  value={editName}
                  onChange={(e) => setEditName(e.target.value)}
                  placeholder='Full name'
                />
              </div>
              <div>
                <Label htmlFor='edit-role'>Role</Label>
                <select
                  id='edit-role'
                  value={editRole}
                  onChange={(e) => setEditRole(e.target.value)}
                  className='border-input bg-background ring-offset-background placeholder:text-muted-foreground focus-visible:ring-ring flex h-10 w-full rounded-md border px-3 py-2 text-sm focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none'
                >
                  <option value='CUSTOMER'>Customer</option>
                  <option value='MANAGER'>Manager</option>
                  <option value='ADMIN'>Admin</option>
                  <option value='SUPER_ADMIN'>Super Admin</option>
                </select>
              </div>
            </div>
          ) : (
            <div className='grid gap-3 md:grid-cols-2 lg:grid-cols-3'>
              {[
                { label: 'Email', value: user.email },
                { label: 'Name', value: user.name || '—' },
                { label: 'Role', value: user.role },
                {
                  label: 'Status',
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
                { label: 'Workspace', value: user.tenantName || '—' },
                {
                  label: 'Member Since',
                  value: new Date(user.createdAt).toLocaleDateString()
                },
                {
                  label: 'ID',
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
