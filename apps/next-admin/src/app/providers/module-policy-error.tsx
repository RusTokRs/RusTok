'use client';

import { useTranslations } from '@rustok/next-fluent/client';
import { useRouter } from 'next/navigation';
import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/shadcn/alert';
import { Button } from '@/shared/ui/shadcn/button';

/** Fail closed on a policy read error without presenting it as a denial. */
export function ModulePolicyError() {
  const t = useTranslations('modules');
  const router = useRouter();

  return (
    <div className='mx-auto w-full max-w-lg space-y-4 p-6'>
      <Alert variant='destructive'>
        <AlertTitle>{t('policy.error.title')}</AlertTitle>
        <AlertDescription>{t('policy.error.description')}</AlertDescription>
      </Alert>
      <Button type='button' variant='outline' onClick={() => router.refresh()}>
        {t('policy.error.retry')}
      </Button>
    </div>
  );
}
