import { NavItem } from '@/types';
import { getAdminNavItems } from '@/modules';

const coreNavItems: NavItem[] = [
  {
    title: 'Dashboard',
    url: '/dashboard',
    i18nKey: 'dashboard',
    group: 'overview',
    icon: 'dashboard',
    isActive: false,
    shortcut: ['d', 'd'],
    items: []
  },
  {
    title: 'Access',
    url: '#',
    i18nKey: 'access',
    group: 'management',
    icon: 'users',
    isActive: false,
    items: [
      {
        title: 'Users',
        url: '/dashboard/users',
        i18nKey: 'users',
        shortcut: ['u', 'u'],
        access: { role: 'manager' }
      },
      {
        title: 'Roles',
        url: '/dashboard/roles',
        i18nKey: 'roles',
        shortcut: ['r', 'p'],
        access: { role: 'admin' }
      }
    ]
  },
  {
    title: 'Platform',
    url: '#',
    i18nKey: 'platform',
    group: 'management',
    icon: 'modules',
    isActive: false,
    items: [
      {
        title: 'Modules',
        url: '/dashboard/modules',
        i18nKey: 'modules',
        shortcut: ['g', 'm'],
        access: { role: 'admin' }
      },
      {
        title: 'App Connections',
        url: '/dashboard/apps',
        i18nKey: 'appConnections',
        shortcut: ['a', 'a'],
        access: { role: 'admin' }
      }
    ]
  },
  {
    title: 'Operations',
    url: '#',
    i18nKey: 'operations',
    group: 'management',
    icon: 'settings',
    isActive: false,
    items: [
      {
        title: 'Search',
        url: '/dashboard/search',
        i18nKey: 'search',
        shortcut: ['s', 's'],
        access: { role: 'admin' }
      },
      {
        title: 'SEO',
        url: '/dashboard/seo',
        shortcut: ['s', 'o'],
        access: { role: 'admin' }
      },
      {
        title: 'AI',
        url: '/dashboard/ai',
        i18nKey: 'ai',
        shortcut: ['a', 'i'],
        access: { role: 'admin' }
      },
      {
        title: 'Email',
        url: '/dashboard/email',
        i18nKey: 'email',
        shortcut: ['e', 'm'],
        access: { role: 'admin' }
      },
      {
        title: 'Cache',
        url: '/dashboard/cache',
        i18nKey: 'cache',
        shortcut: ['c', 'h'],
        access: { role: 'admin' }
      },
      {
        title: 'Events',
        url: '/dashboard/events',
        i18nKey: 'events',
        access: { role: 'admin' }
      }
    ]
  },
  {
    title: 'Account',
    url: '#',
    i18nKey: 'account',
    group: 'account',
    icon: 'account',
    isActive: true,
    items: [
      {
        title: 'Profile',
        url: '/dashboard/profile',
        i18nKey: 'profile',
        icon: 'profile',
        shortcut: ['m', 'm']
      }
    ]
  }
];

export const navItems: NavItem[] = [...coreNavItems, ...getAdminNavItems()];
