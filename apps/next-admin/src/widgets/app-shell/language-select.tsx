'use client';

import {
  LOCALE_COOKIE,
  localeLabels,
  locales,
  type Locale
} from '@/i18n/config';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import { IconLanguage } from '@tabler/icons-react';
import { useLocale, useTranslations } from 'next-intl';
import { usePathname, useRouter, useSearchParams } from 'next/navigation';

const COOKIE_MAX_AGE_SECONDS = 60 * 60 * 24 * 365;

function isLocale(value: string): value is Locale {
  return locales.includes(value as Locale);
}

function persistLocale(locale: Locale) {
  document.cookie = `${LOCALE_COOKIE}=${locale}; Path=/; Max-Age=${COOKIE_MAX_AGE_SECONDS}; SameSite=Lax`;
}

export function LanguageSelect() {
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();
  const currentLocale = useLocale();
  const t = useTranslations('app.nav');
  const value = isLocale(currentLocale) ? currentLocale : 'en';

  const handleLocaleChange = (nextLocale: string) => {
    if (!isLocale(nextLocale) || nextLocale === value) return;

    persistLocale(nextLocale);

    const params = new URLSearchParams(searchParams.toString());
    params.delete('locale');
    const query = params.toString();

    router.replace(query ? `${pathname}?${query}` : pathname, {
      scroll: false
    });
    router.refresh();
  };

  return (
    <Select value={value} onValueChange={handleLocaleChange}>
      <SelectTrigger size='sm' aria-label={t('language')} className='min-w-32'>
        <IconLanguage className='size-4' />
        <SelectValue />
      </SelectTrigger>
      <SelectContent align='end'>
        {locales.map((locale) => (
          <SelectItem key={locale} value={locale}>
            {localeLabels[locale]}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
