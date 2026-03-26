import type { AdminNavItem } from '@/modules/types';

export const eventsNavItems: AdminNavItem[] = [
  {
    group: 'Infrastructure',
    label: 'Events & Outbox',
    href: '/dashboard/events',
    icon: 'bolt'
  }
];
