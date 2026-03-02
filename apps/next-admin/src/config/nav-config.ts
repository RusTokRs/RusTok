import { NavItem } from '@/types';
import { getAdminNavItems } from '@/modules';

const coreNavItems: NavItem[] = [
  {
    title: 'Dashboard',
    url: '/dashboard/overview',
    icon: 'dashboard',
    isActive: false,
    shortcut: ['d', 'd'],
    items: []
  },
  {
    title: 'Users',
    url: '/dashboard/users',
    icon: 'users',
    shortcut: ['u', 'u'],
    isActive: false,
    items: [],
    access: { role: 'manager' }
  },
  {
    title: 'Product',
    url: '/dashboard/product',
    icon: 'product',
    shortcut: ['p', 'p'],
    isActive: false,
    items: []
  },
  {
    title: 'Kanban',
    url: '/dashboard/kanban',
    icon: 'kanban',
    shortcut: ['k', 'k'],
    isActive: false,
    items: []
  },
  {
    title: 'Account',
    url: '#',
    icon: 'account',
    isActive: true,
    items: [
      {
        title: 'Profile',
        url: '/dashboard/profile',
        icon: 'profile',
        shortcut: ['m', 'm']
      }
    ]
  }
];

export const navItems: NavItem[] = [
  ...coreNavItems,
  ...getAdminNavItems()
];
