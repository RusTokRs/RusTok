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
      }
    ]
  }
];
