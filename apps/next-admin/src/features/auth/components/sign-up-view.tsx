import { cn } from '@/shared/lib/utils';
import Link from 'next/link';
import { InteractiveGridPattern } from './interactive-grid';
import UserRegisterForm from './user-register-form';
import { getTranslations } from 'next-intl/server';
import { LanguageSelect } from '@/widgets/app-shell/language-select';

export default async function SignUpViewPage() {
  const t = await getTranslations('register');

  return (
    <div className='relative h-screen flex-col items-center justify-center md:grid lg:max-w-none lg:grid-cols-2 lg:px-0'>
      <div className='bg-muted relative hidden h-full flex-col p-10 text-white lg:flex dark:border-r'>
        <div className='absolute inset-0 bg-zinc-900' />
        <div className='relative z-20 flex items-center text-lg font-medium'>
          <svg
            xmlns='http://www.w3.org/2000/svg'
            viewBox='0 0 24 24'
            fill='none'
            stroke='currentColor'
            strokeWidth='2'
            strokeLinecap='round'
            strokeLinejoin='round'
            className='mr-2 h-6 w-6'
          >
            <path d='M15 6v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3V6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3' />
          </svg>
          RusTok Admin
        </div>
        <InteractiveGridPattern
          className={cn(
            'mask-[radial-gradient(400px_circle_at_center,white,transparent)]',
            'inset-x-0 inset-y-[0%] h-full skew-y-12'
          )}
        />
        <div className='relative z-20 mt-auto'>
          <blockquote className='space-y-2'>
            <p className='text-lg'>
              "RusTok - a multi-tenant language agnostic highload platform built with Rust and
              WebAssembly."
            </p>
            <footer className='text-sm'>RusTok Team</footer>
          </blockquote>
        </div>
      </div>
      <div className='flex h-full items-center justify-center p-4 lg:p-8'>
        <div className='flex w-full max-w-md flex-col items-center justify-center space-y-6'>
          <div className='flex flex-col space-y-2 text-center'>
            <h1 className='text-2xl font-semibold tracking-tight'>
              {t('title')}
            </h1>
            <p className='text-muted-foreground text-sm'>{t('subtitle')}</p>
          </div>
          <LanguageSelect />
          <UserRegisterForm />
          <p className='text-muted-foreground px-8 text-center text-sm'>
            <Link
              href='/auth/sign-in'
              className='hover:text-primary underline underline-offset-4'
            >
              {t('loginLink')}
            </Link>
          </p>
        </div>
      </div>
    </div>
  );
}
