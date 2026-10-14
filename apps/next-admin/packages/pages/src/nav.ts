import type { NavItem } from '@/types';

export const pagesNavItems: NavItem[] = [
  {
    title: 'Pages',
    url: '#',
    i18nKey: 'pages',
    group: 'modulePlugins',
    icon: 'page',
    isActive: true,
    items: [
      {
        title: 'All Pages',
        url: '/dashboard/pages',
        i18nKey: 'pages',
        shortcut: ['p', 'l']
      }
    ]
  }
];
