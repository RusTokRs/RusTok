import type { NavItem } from '@/types';

export const cacheNavItems: NavItem[] = [
  {
    title: 'System',
    url: '#',
    icon: 'dashboard',
    isActive: false,
    items: [
      {
        title: 'Cache',
        url: '/dashboard/cache'
      }
    ],
    access: { role: 'admin' }
  }
];
