'use client';
import { navItems } from '@/shared/config/nav-config';
import {
  KBarAnimator,
  KBarPortal,
  KBarPositioner,
  KBarProvider,
  KBarSearch
} from 'kbar';
import { useTranslations } from '@rustok/next-fluent';
import { useRouter } from 'next/navigation';
import { useMemo } from 'react';
import RenderResults from './render-result';
import AdminGlobalSearchActions from './admin-global-search';
import useThemeSwitching from './use-theme-switching';
import { useFilteredNavItems } from '@/shared/hooks/use-nav';
import type { NavItem } from '@/shared/types';

function getNavLabel(t: ReturnType<typeof useTranslations>, item: NavItem) {
  return item.i18nKey ? t(item.i18nKey) : item.title;
}

function KBar({ children }: { children: React.ReactNode }) {
  const router = useRouter();
  const filteredItems = useFilteredNavItems(navItems);
  const tNav = useTranslations('app.nav');
  const tCommand = useTranslations('app.command');

  // These actions are for the navigation.
  const actions = useMemo(() => {
    const navigateTo = (url: string) => {
      router.push(url);
    };

    const collectActions = (items: NavItem[], section: string): any[] =>
      items.flatMap((item) => {
        const label = getNavLabel(tNav, item);
        const action =
          item.url !== '#'
            ? {
                id: `${item.url}-${item.title}-action`,
                name: label,
                shortcut: item.shortcut,
                keywords: `${item.title} ${label}`.toLowerCase(),
                section,
                subtitle: `${tCommand('goTo')} ${label}`,
                perform: () => navigateTo(item.url)
              }
            : null;

        const childActions = item.items?.length
          ? collectActions(item.items, label)
          : [];

        return action ? [action, ...childActions] : childActions;
      });

    return collectActions(filteredItems, tCommand('navigation'));
  }, [router, filteredItems, tCommand, tNav]);

  return (
    <KBarProvider actions={actions}>
      <KBarComponent>{children}</KBarComponent>
    </KBarProvider>
  );
}
const KBarComponent = ({ children }: { children: React.ReactNode }) => {
  useThemeSwitching();

  return (
    <>
      <AdminGlobalSearchActions />
      <KBarPortal>
        <KBarPositioner className='bg-background/80 fixed inset-0 z-99999 p-0! backdrop-blur-sm'>
          <KBarAnimator className='bg-card text-card-foreground relative mt-64! w-full max-w-[600px] -translate-y-12! overflow-hidden rounded-lg border shadow-lg'>
            <div className='bg-card border-border sticky top-0 z-10 border-b'>
              <KBarSearch className='bg-card w-full border-none px-6 py-4 text-lg outline-hidden focus:ring-0 focus:ring-offset-0 focus:outline-hidden' />
            </div>
            <div className='max-h-[400px]'>
              <RenderResults />
            </div>
          </KBarAnimator>
        </KBarPositioner>
      </KBarPortal>
      {children}
    </>
  );
};

export { KBar };
export default KBar;
