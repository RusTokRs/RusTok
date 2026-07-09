import type { NavItem } from '@/types';

export const eventsNavItems: NavItem[] = [
  {
    title: 'Infrastructure',
    url: '#',
    icon: 'dashboard',
    isActive: false,
    items: [
      {
        title: 'Events & Outbox',
        url: '/dashboard/events'
      }
    ],
    access: { role: 'admin' }
  }
];
