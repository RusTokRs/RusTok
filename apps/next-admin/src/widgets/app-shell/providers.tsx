'use client';
import React from 'react';
import { SessionProvider } from 'next-auth/react';
import type { Session } from 'next-auth';
import { ActiveThemeProvider } from '@/shared/lib/themes/active-theme';

export default function Providers({
  activeThemeValue,
  session,
  children
}: {
  activeThemeValue: string;
  session: Session | null;
  children: React.ReactNode;
}) {
  return (
    <ActiveThemeProvider initialTheme={activeThemeValue}>
      <SessionProvider session={session}>{children}</SessionProvider>
    </ActiveThemeProvider>
  );
}
