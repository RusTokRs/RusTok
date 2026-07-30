import type { NavItem } from '@/types';

export const forumNavItems: NavItem[] = [
  {
    title: 'Forum',
    url: '#',
    i18nKey: 'forum',
    group: 'modulePlugins',
    icon: 'blog',
    items: [
      {
        title: 'Reply Composer',
        url: '/dashboard/forum/reply',
        i18nKey: 'replyComposer',
        shortcut: ['f', 'r']
      }
    ]
  }
];
