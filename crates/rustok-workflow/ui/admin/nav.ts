import type { NavItem } from '@/types';

export const workflowNavItems: NavItem[] = [
  {
    title: 'Workflows',
    url: '#',
    icon: 'workflow',
    isActive: false,
    items: [
      {
        title: 'All Workflows',
        url: '/dashboard/workflows',
        shortcut: ['w', 'l']
      },
      {
        title: 'New Workflow',
        url: '/dashboard/workflows/new',
        shortcut: ['w', 'n']
      }
    ]
  }
];
