import type { NavItem } from '@/types';

export const rbacNavItems: NavItem[] = [
  {
    title: 'Access Control',
    url: '#',
    icon: 'modules',
    isActive: false,
    items: [
      {
        title: 'Roles & Permissions',
        url: '/dashboard/roles',
        shortcut: ['r', 'p']
      }
    ],
    access: { role: 'admin' }
  }
];
