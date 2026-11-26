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
import { signOut, useSession } from 'next-auth/react';
import { useState } from 'react';
import { toast } from 'sonner';
import { SessionsCard } from './sessions-card';

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

const REQUEST_EMAIL_CHANGE_MUTATION = `
mutation RequestEmailChange($input: RequestEmailChangeInput!) {
  requestEmailChange(input: $input) {
    success
  }
}`;

const CONFIRM_EMAIL_CHANGE_MUTATION = `
mutation ConfirmEmailChange($input: ConfirmEmailChangeInput!) {
  confirmEmailChange(input: $input) {
    success
    email
  }
}`;

export default function ProfileViewPage() {
  const { data: session, update } = useSession();
  const user = session?.user;
  const tenantSlug = user?.tenantSlug;

  const [name, setName] = useState(user?.name ?? '');
  const [isSavingProfile, setIsSavingProfile] = useState(false);

  const [newEmail, setNewEmail] = useState('');
  const [emailChangePassword, setEmailChangePassword] = useState('');
  const [emailChangeToken, setEmailChangeToken] = useState('');
  const [isRequestingEmailChange, setIsRequestingEmailChange] = useState(false);
  const [isConfirmingEmailChange, setIsConfirmingEmailChange] = useState(false);

  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [isChangingPassword, setIsChangingPassword] = useState(false);

  const handleSaveProfile = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!user) return;
    setIsSavingProfile(true);
    try {
      await graphqlRequest<object, unknown>(
        UPDATE_PROFILE_MUTATION,
        { input: { name: name.trim() || null } },
        undefined,
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

  const handleRequestEmailChange = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!user || !tenantSlug) return;
    if (!newEmail.trim() || !emailChangePassword) {
      toast.error('Enter a new email address and your current password');
      return;
    }

    setIsRequestingEmailChange(true);
    try {
      await graphqlRequest<object, { requestEmailChange: { success: boolean } }>(
        REQUEST_EMAIL_CHANGE_MUTATION,
        {
          input: {
            newEmail: newEmail.trim(),
            currentPassword: emailChangePassword
          }
        },
        undefined,
        tenantSlug
      );
      setEmailChangePassword('');
      toast.success('A confirmation token was sent to the new email address');
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : 'Failed to request email change'
      );
    } finally {
      setIsRequestingEmailChange(false);
    }
  };

  const handleConfirmEmailChange = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!user || !tenantSlug) return;
    if (!emailChangeToken.trim()) {
      toast.error('Enter the confirmation token from your email');
      return;
    }

    setIsConfirmingEmailChange(true);
    try {
      const result = await graphqlRequest<
        object,
        { confirmEmailChange: { success: boolean; email: string } }
      >(
        CONFIRM_EMAIL_CHANGE_MUTATION,
        { input: { token: emailChangeToken.trim() } },
        undefined,
        tenantSlug
      );
      if (!result.confirmEmailChange.success) {
        throw new Error('Email change confirmation was not accepted');
      }
      toast.success(
        'Email changed to ' + result.confirmEmailChange.email +
        '. Sign in again with the new address.'
      );
      await signOut({ callbackUrl: '/auth/sign-in' });
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : 'Failed to confirm email change'
      );
    } finally {
      setIsConfirmingEmailChange(false);
    }
  };

  const handleChangePassword = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!user) return;
    if (!currentPassword || !newPassword) {
      toast.error('Both fields are required');
      return;
    }
    setIsChangingPassword(true);
    try {
      await graphqlRequest<object, unknown>(
        CHANGE_PASSWORD_MUTATION,
        { input: { currentPassword, newPassword } },
        undefined,
        tenantSlug
      );
      setCurrentPassword('');
      setNewPassword('');
      toast.success('Password changed. Signing out of this session.');
      await signOut({ callbackUrl: '/auth/sign-in' });
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
              <CardTitle>Change Email</CardTitle>
              <CardDescription>
                Confirm ownership of the new mailbox. Your current address remains active until confirmation.
              </CardDescription>
            </CardHeader>
            <CardContent className='space-y-5'>
              <form onSubmit={handleRequestEmailChange} className='space-y-4'>
                <div>
                  <Label htmlFor='new-email'>New Email Address</Label>
                  <Input
                    id='new-email'
                    type='email'
                    autoComplete='email'
                    value={newEmail}
                    onChange={(e) => setNewEmail(e.target.value)}
                    placeholder='name@example.com'
                    disabled={isRequestingEmailChange || isConfirmingEmailChange}
                  />
                </div>
                <div>
                  <Label htmlFor='email-change-password'>Current Password</Label>
                  <Input
                    id='email-change-password'
                    type='password'
                    autoComplete='current-password'
                    value={emailChangePassword}
                    onChange={(e) => setEmailChangePassword(e.target.value)}
                    placeholder='********'
                    disabled={isRequestingEmailChange || isConfirmingEmailChange}
                  />
                </div>
                <Button type='submit' disabled={isRequestingEmailChange || isConfirmingEmailChange}>
                  {isRequestingEmailChange ? 'Sending...' : 'Send confirmation token'}
                </Button>
              </form>
              <form onSubmit={handleConfirmEmailChange} className='space-y-4 border-t pt-4'>
                <div>
                  <Label htmlFor='email-change-token'>Confirmation Token</Label>
                  <Input
                    id='email-change-token'
                    value={emailChangeToken}
                    onChange={(e) => setEmailChangeToken(e.target.value)}
                    placeholder='Paste the token from your email'
                    autoComplete='one-time-code'
                    disabled={isRequestingEmailChange || isConfirmingEmailChange}
                  />
                </div>
                <Button type='submit' disabled={isRequestingEmailChange || isConfirmingEmailChange}>
                  {isConfirmingEmailChange ? 'Confirming...' : 'Confirm new email'}
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

          <SessionsCard tenantSlug={tenantSlug} />
        </div>
      ) : (
        <p className='text-muted-foreground text-sm'>Loading profile...</p>
      )}
    </PageContainer>
  );
}
