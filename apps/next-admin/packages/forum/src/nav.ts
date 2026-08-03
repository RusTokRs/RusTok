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
      title: 'Merge Topics',
      url: '/dashboard/forum/merge',
      icon: 'messageSquare',
      moduleSlug: 'forum',
      access: { permission: 'forum_topics:manage' }
    }
  ]
};
