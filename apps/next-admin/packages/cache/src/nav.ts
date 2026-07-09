import type { NavItem } from '@/types';

export const cacheNavItems: NavItem[] = [
  {
    title: 'Infrastructure',
    url: '#',
    icon: 'dashboard',
    isActive: false,
    items: [
      {
        title: 'Cache',
        url: '/dashboard/cache',
        shortcut: ['c', 'h']
      }
    ],
    access: { role: 'admin' }
  }
];
