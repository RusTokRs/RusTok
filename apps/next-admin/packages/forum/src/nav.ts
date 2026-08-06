import type { NavItem } from '@/shared/types';

export const forumNav: NavItem = {
  title: 'Forum',
  url: '/dashboard/forum',
  i18nKey: 'forum',
  icon: 'messageSquare',
  group: 'modulePlugins',
  moduleSlug: 'forum',
  items: [
    {
      title: 'Reply Composer',
      url: '/dashboard/forum/reply',
      i18nKey: 'replyComposer',
      icon: 'messageSquare',
      moduleSlug: 'forum'
    },
    {
      title: 'Rename Topic Route',
      url: '/dashboard/forum/rename-slug',
      icon: 'messageSquare',
      moduleSlug: 'forum',
      access: { permission: 'forum_topics:update' }
    },
    {
      title: 'Merge Topics',
      url: '/dashboard/forum/merge',
      icon: 'messageSquare',
      moduleSlug: 'forum',
      access: { permission: 'forum_topics:manage' }
    },
    {
      title: 'Fork Reply Branch',
      url: '/dashboard/forum/fork',
      icon: 'messageSquare',
      moduleSlug: 'forum',
      access: { permission: 'forum_topics:manage' }
    },
    {
      title: 'Move Reply Range',
      url: '/dashboard/forum/reply-range',
      icon: 'messageSquare',
      moduleSlug: 'forum',
      access: { permission: 'forum_topics:manage' }
    },
    {
      title: 'Split Topic',
      url: '/dashboard/forum/split',
      icon: 'messageSquare',
      moduleSlug: 'forum',
      access: { permission: 'forum_topics:manage' }
    }
  ]
};