import type { NavItem } from '@/types';

export const searchNavItems: NavItem[] = [
  {
    title: 'Search',
    url: '/dashboard/search',
    i18nKey: 'search',
    group: 'modulePlugins',
    icon: 'search',
    isActive: false,
    items: []
  }
];
