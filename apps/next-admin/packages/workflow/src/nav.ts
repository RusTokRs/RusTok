import type { NavItem } from '@/types';

export const workflowNavItems: NavItem[] = [
  {
    title: 'Workflows',
    url: '#',
    i18nKey: 'workflows',
    group: 'modulePlugins',
    icon: 'workflow',
    isActive: false,
    items: [
      {
        title: 'All Workflows',
        url: '/dashboard/workflows',
        i18nKey: 'allWorkflows',
        shortcut: ['w', 'l']
      },
      {
        title: 'New Workflow',
        url: '/dashboard/workflows/new',
        i18nKey: 'newWorkflow',
        shortcut: ['w', 'n']
      }
    ]
  }
];
