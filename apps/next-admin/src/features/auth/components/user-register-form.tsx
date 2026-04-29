'use client';
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import { signUp } from '@/shared/api/auth-api';
import { signIn } from 'next-auth/react';
import { useRouter } from 'next/navigation';
import { useState } from 'react';
import { toast } from 'sonner';

export default function UserRegisterForm() {
  const router = useRouter();
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [tenantSlug, setTenantSlug] = useState('demo');
  const [isLoading, setIsLoading] = useState(false);

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!email || !password || !tenantSlug) {
      toast.error('Please fill in all required fields');
      return;
    }
    setIsLoading(true);
    try {
      // Регистрируем через GraphQL
      await signUp(
        email.trim(),
        password,
        tenantSlug.trim(),
        name.trim() || undefined
      );
      // Затем входим через NextAuth (чтобы получить сессию)
      const result = await signIn('credentials', {
        email: email.trim(),
        password,
        tenantSlug: tenantSlug.trim(),
        redirect: false
      });
      if (result?.error) {
        toast.error(
          'Account created but sign-in failed. Please sign in manually.'
        );
        router.push('/auth/sign-in');
      } else {
        toast.success('Account created successfully');
        router.push('/dashboard/overview');
        router.refresh();
      }
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Registration failed');
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <form onSubmit={onSubmit} className='w-full space-y-4'>
      <div className='space-y-2'>
        <Label htmlFor='tenant'>Workspace</Label>
        <Input
          id='tenant'
          placeholder='demo'
          value={tenantSlug}
          onChange={(e) => setTenantSlug(e.target.value)}
          disabled={isLoading}
          required
        />
      </div>
      <div className='space-y-2'>
        <Label htmlFor='name'>Name (optional)</Label>
        <Input
          id='name'
          placeholder='Your Name'
          value={name}
          onChange={(e) => setName(e.target.value)}
          disabled={isLoading}
        />
      </div>
      <div className='space-y-2'>
        <Label htmlFor='email'>Email</Label>
        <Input
          id='email'
          type='email'
          placeholder='admin@rustok.io'
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          disabled={isLoading}
          required
        />
      </div>
      <div className='space-y-2'>
        <Label htmlFor='password'>Password</Label>
        <Input
          id='password'
          type='password'
          placeholder='••••••••'
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          disabled={isLoading}
          required
        />
      </div>
      <Button type='submit' className='w-full' disabled={isLoading}>
        {isLoading ? 'Creating account...' : 'Create Account'}
      </Button>
    </form>
  );
}
