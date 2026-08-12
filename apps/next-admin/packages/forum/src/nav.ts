import type { NavItem } from '@/shared/types';

export const forumNav: NavItem = {
  title: 'Forum',
  url: '/dashboard/forum',
  i18nKey: 'forum',
  icon: 'forum',
  group: 'modulePlugins',
  moduleSlug: 'forum',
  items: [
    {
      title: 'Topic Composer',
      url: '/dashboard/forum/topic',
      i18nKey: 'topicComposer',
      icon: 'forum',
      moduleSlug: 'forum'
    },
    {
      title: 'Reply Composer',
      url: '/dashboard/forum/reply',
      i18nKey: 'replyComposer',
      icon: 'forum',
      moduleSlug: 'forum'
    },
    {
      title: 'Rename Topic Route',
      url: '/dashboard/forum/rename-slug',
      icon: 'forum',
      moduleSlug: 'forum',
      access: { permission: 'forum_topics:update' }
    },
    {
      title: 'Merge Topics',
      url: '/dashboard/forum/merge',
      icon: 'forum',
      moduleSlug: 'forum',
      access: { permission: 'forum_topics:manage' }
    },
    {
      title: 'Fork Reply Branch',
      url: '/dashboard/forum/fork',
      icon: 'forum',
      moduleSlug: 'forum',
      access: { permission: 'forum_topics:manage' }
    },
    {
      title: 'Move Reply Range',
      url: '/dashboard/forum/reply-range',
      icon: 'forum',
      moduleSlug: 'forum',
      access: { permission: 'forum_topics:manage' }
    },
    {
      title: 'Split Topic',
      url: '/dashboard/forum/split',
      icon: 'forum',
      moduleSlug: 'forum',
      access: { permission: 'forum_topics:manage' }
    }
  ]
};
