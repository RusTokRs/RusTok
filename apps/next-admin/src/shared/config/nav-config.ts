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
    title: 'App Connections',
    url: '/dashboard/apps',
    icon: 'post',
    shortcut: ['a', 'a'],
    isActive: false,
    items: [],
    access: { role: 'admin' }
  },
  {
    title: 'Modules',
    url: '/dashboard/modules',
    icon: 'modules',
    shortcut: ['g', 'm'],
    isActive: false,
    items: [],
    access: { role: 'admin' }
  },
  {
    title: 'Search',
    url: '/dashboard/search',
    icon: 'search',
    shortcut: ['s', 's'],
    isActive: false,
    items: [],
    access: { role: 'admin' }
  },
  {
    title: 'AI',
    url: '/dashboard/ai',
    icon: 'modules',
    shortcut: ['a', 'i'],
    isActive: false,
    items: [],
    access: { role: 'admin' }
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

export const navItems: NavItem[] = [...coreNavItems, ...getAdminNavItems()];
