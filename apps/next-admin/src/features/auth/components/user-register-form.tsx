'use client';
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import { signUp } from '@/shared/api/auth-api';
import { signIn } from 'next-auth/react';
import { useRouter } from 'next/navigation';
import { useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from '@rustok/next-fluent';

export default function UserRegisterForm() {
  const t = useTranslations('register');
  const router = useRouter();
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [tenantSlug, setTenantSlug] = useState('demo');
  const [isLoading, setIsLoading] = useState(false);

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!email || !password || !tenantSlug) {
      toast.error(t('errorRequired'));
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
        toast.success(t('success'));
        router.push('/dashboard');
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
        <Label htmlFor='tenant'>{t('tenantLabel')}</Label>
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
        <Label htmlFor='name'>{t('nameLabel')}</Label>
        <Input
          id='name'
          placeholder='Your Name'
          value={name}
          onChange={(e) => setName(e.target.value)}
          disabled={isLoading}
        />
      </div>
      <div className='space-y-2'>
        <Label htmlFor='email'>{t('emailLabel')}</Label>
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
        <Label htmlFor='password'>{t('passwordLabel')}</Label>
        <Input
          id='password'
          type='password'
          placeholder='********'
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          disabled={isLoading}
          required
        />
      </div>
      <Button type='submit' className='w-full' disabled={isLoading}>
        {isLoading ? `${t('submit')}...` : t('submit')}
      </Button>
    </form>
  );
}
