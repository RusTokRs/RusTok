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
      }
    ]
  }
];
