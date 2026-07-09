import type { NavItem } from '@/types';

export const blogNavItems: NavItem[] = [
  {
    title: 'Blog',
    url: '#',
    i18nKey: 'blog',
    group: 'modulePlugins',
    icon: 'blog',
    isActive: true,
    items: [
      {
        title: 'Posts',
        url: '/dashboard/blog',
        i18nKey: 'posts',
        shortcut: ['b', 'p']
      },
      {
        title: 'New Post',
        url: '/dashboard/blog/new',
        i18nKey: 'newPost',
        shortcut: ['b', 'n']
      },
      {
        title: 'Page Builder',
        url: '/dashboard/blog/page-builder',
        i18nKey: 'pageBuilder',
        shortcut: ['b', 'g']
      }
    ]
  }
];

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
