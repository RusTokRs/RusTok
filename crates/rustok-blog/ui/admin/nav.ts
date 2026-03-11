import type { NavItem } from '@/types';

export const blogNavItems: NavItem[] = [
  {
    title: 'Blog',
    url: '#',
    icon: 'blog',
    isActive: true,
    items: [
      {
        title: 'Posts',
        url: '/dashboard/blog',
        shortcut: ['b', 'p']
      },
      {
        title: 'New Post',
        url: '/dashboard/blog/new',
        shortcut: ['b', 'n']
      },
      {
        title: 'Page Builder',
        url: '/dashboard/blog/page-builder',
        shortcut: ['b', 'g']
      },
      {
        title: 'Forum Reply',
        url: '/dashboard/blog/forum-reply',
        shortcut: ['b', 'f']
      }
    ]
  }
];
