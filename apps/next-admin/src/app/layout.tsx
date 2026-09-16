import { Providers } from '@/widgets/app-shell';
import { auth } from '@/auth';
import { Toaster } from '@/shared/ui/shadcn/sonner';
import { fontVariables } from '@/shared/lib/themes/font.config';
import { DEFAULT_THEME } from '@/shared/lib/themes/theme.config';
import ThemeProvider from '@/shared/lib/themes/theme-provider';
import { cn } from '@/shared/lib/utils';
import type { Metadata, Viewport } from 'next';
import { cookies } from 'next/headers';
import { FluentProvider } from '@rustok/next-fluent/client';
import { getLocale, getMessages } from '@rustok/next-fluent/server';
import '@/i18n/request';
import NextTopLoader from 'nextjs-toploader';
import { NuqsAdapter } from 'nuqs/adapters/next/app';
import '../styles/globals.css';

const META_THEME_COLORS = {
  light: '#ffffff',
  dark: '#09090b'
};

export const metadata: Metadata = {
  title: 'Next Shadcn',
  description: 'Basic dashboard with Next.js and Shadcn'
};

export const viewport: Viewport = {
  themeColor: META_THEME_COLORS.light
};

export default async function RootLayout({
  children
}: {
  children: React.ReactNode;
}) {
  const cookieStore = await cookies();
  const activeThemeValue = cookieStore.get('active_theme')?.value;
  const themeToApply = activeThemeValue || DEFAULT_THEME;
  const session = await auth();
  const locale = await getLocale();
  const messages = await getMessages();

  return (
    <html lang={locale} suppressHydrationWarning data-theme={themeToApply}>
      <head>
        <script
          dangerouslySetInnerHTML={{
            __html: `
              try {
                // Set meta theme color
                if (localStorage.theme === 'dark' || ((!('theme' in localStorage) || localStorage.theme === 'system') && window.matchMedia('(prefers-color-scheme: dark)').matches)) {
                  document.querySelector('meta[name="theme-color"]')?.setAttribute('content', '${META_THEME_COLORS.dark}')
                }
              } catch (_) {}
            `
          }}
        />
      </head>
      <body
        className={cn(
          'bg-background overflow-hidden overscroll-none font-sans antialiased',
          fontVariables
        )}
      >
        <NextTopLoader color='var(--primary)' showSpinner={false} />
        <FluentProvider messages={messages} locale={locale}>
          <NuqsAdapter>
            <ThemeProvider
              attribute='class'
              defaultTheme='system'
              enableSystem
              disableTransitionOnChange
              enableColorScheme
            >
              <Providers activeThemeValue={themeToApply} session={session}>
                <Toaster />
                {children}
              </Providers>
            </ThemeProvider>
          </NuqsAdapter>
        </FluentProvider>
      </body>
    </html>
  );
}
