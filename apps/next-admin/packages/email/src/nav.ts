import type { NavItem } from '@/types';

export const emailNavItems: NavItem[] = [
  {
    title: 'Platform Settings',
    url: '#',
    icon: 'settings',
    isActive: false,
    items: [
      {
        title: 'Email',
        url: '/dashboard/email',
        shortcut: ['e', 'm']
      }
    ],
    access: { role: 'admin' }
  }
];
