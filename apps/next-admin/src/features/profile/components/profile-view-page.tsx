'use client';
import { PageContainer } from '@/widgets/app-shell';
import { Button } from '@/shared/ui/shadcn/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import { graphqlRequest } from '@/shared/api/graphql';
import { useSession } from 'next-auth/react';
import { useState } from 'react';
import { toast } from 'sonner';

const UPDATE_PROFILE_MUTATION = `
mutation UpdateProfile($input: UpdateProfileInput!) {
  updateProfile(input: $input) {
    id email name role status
  }
}`;

const CHANGE_PASSWORD_MUTATION = `
mutation ChangePassword($input: ChangePasswordInput!) {
  changePassword(input: $input) {
    success
  }
}`;

export default function ProfileViewPage() {
  const { data: session, update } = useSession();
  const user = session?.user;
  const token = user?.rustokToken;
  const tenantSlug = user?.tenantSlug;

  const [name, setName] = useState(user?.name ?? '');
  const [isSavingProfile, setIsSavingProfile] = useState(false);

  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [isChangingPassword, setIsChangingPassword] = useState(false);

  const handleSaveProfile = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!token) return;
    setIsSavingProfile(true);
    try {
      await graphqlRequest<object, unknown>(
        UPDATE_PROFILE_MUTATION,
        { input: { name: name.trim() || null } },
        token,
        tenantSlug
      );
      await update({ name: name.trim() || undefined });
      toast.success('Profile updated');
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : 'Failed to update profile'
      );
    } finally {
      setIsSavingProfile(false);
    }
  };

  const handleChangePassword = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!token) return;
    if (!currentPassword || !newPassword) {
      toast.error('Both fields are required');
      return;
    }
    setIsChangingPassword(true);
    try {
      await graphqlRequest<object, unknown>(
        CHANGE_PASSWORD_MUTATION,
        { input: { currentPassword, newPassword } },
        token,
        tenantSlug
      );
      setCurrentPassword('');
      setNewPassword('');
      toast.success('Password changed. You will need to sign in again.');
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : 'Failed to change password'
      );
    } finally {
      setIsChangingPassword(false);
    }
  };

  return (
    <PageContainer
      pageTitle='Profile'
      pageDescription='Manage account information and password settings.'
    >
      {user ? (
        <div className='grid gap-4 md:grid-cols-2'>
          <Card>
            <CardHeader>
              <CardTitle>Account Information</CardTitle>
              <CardDescription>
                Update the profile data used across the admin workspace.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <form onSubmit={handleSaveProfile} className='space-y-4'>
                <div>
                  <Label htmlFor='profile-name'>Name</Label>
                  <Input
                    id='profile-name'
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                    placeholder='Your name'
                    disabled={isSavingProfile}
                  />
                </div>
                <div>
                  <p className='text-muted-foreground text-xs font-medium tracking-wider uppercase'>
                    Email
                  </p>
                  <p className='mt-1 text-sm font-medium'>{user.email}</p>
                </div>
                <div>
                  <p className='text-muted-foreground text-xs font-medium tracking-wider uppercase'>
                    Role
                  </p>
                  <p className='mt-1 text-sm font-medium'>{user.role}</p>
                </div>
                <div>
                  <p className='text-muted-foreground text-xs font-medium tracking-wider uppercase'>
                    Workspace
                  </p>
                  <p className='mt-1 text-sm font-medium'>
                    {user.tenantSlug || '-'}
                  </p>
                </div>
                <Button type='submit' disabled={isSavingProfile}>
                  {isSavingProfile ? 'Saving...' : 'Save changes'}
                </Button>
              </form>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Change Password</CardTitle>
              <CardDescription>
                Keep credentials current for the active admin account.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <form onSubmit={handleChangePassword} className='space-y-4'>
                <div>
                  <Label htmlFor='current-password'>Current Password</Label>
                  <Input
                    id='current-password'
                    type='password'
                    value={currentPassword}
                    onChange={(e) => setCurrentPassword(e.target.value)}
                    placeholder='********'
                    disabled={isChangingPassword}
                  />
                </div>
                <div>
                  <Label htmlFor='new-password'>New Password</Label>
                  <Input
                    id='new-password'
                    type='password'
                    value={newPassword}
                    onChange={(e) => setNewPassword(e.target.value)}
                    placeholder='********'
                    disabled={isChangingPassword}
                  />
                </div>
                <Button type='submit' disabled={isChangingPassword}>
                  {isChangingPassword ? 'Updating...' : 'Update password'}
                </Button>
              </form>
            </CardContent>
          </Card>
        </div>
      ) : (
        <p className='text-muted-foreground text-sm'>Loading profile...</p>
      )}
    </PageContainer>
  );
}
