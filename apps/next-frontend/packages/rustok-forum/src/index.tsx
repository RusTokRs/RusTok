import { registerStorefrontModule } from '@/modules/registry';
import { ForumSection } from './components/forum-section';

export type {
  ForumCategoryListItem,
  ForumTopicListItem,
  ForumTopicDetail,
  ForumReplyDetail,
  ForumMemberCard,
  ForumQuoteReference,
  CreateForumReplyWithQuotesInput,
  CreateForumTopicWithQuotesInput,
} from './api/forum';

export {
  fetchStorefrontCategories,
  fetchStorefrontTopics,
  fetchStorefrontTopic,
  fetchStorefrontReplies,
  fetchForumMemberCards,
  markForumTopicRead,
  createForumReply,
  createForumTopic,
} from './api/forum';

export { ForumSection } from './components/forum-section';
export { Composer } from './components/composer';
export { QuoteTooltip } from './components/quote-tooltip';
export { TimelineScroller } from './components/timeline-scroller';
export { CategoryRail } from './components/category-rail';
export { TopicFeed } from './components/topic-feed';
export { ThreadPanel } from './components/thread-panel';
export { CategoryOverview } from './components/category-overview';
export { AuthorBadge, MemberCardProvider } from './components/member-card';
export { ComposerProvider, useComposer } from './context/composer-context';

registerStorefrontModule({
  id: 'forum-discussions',
  moduleSlug: 'forum',
  slot: 'home:afterHero',
  order: 30,
  render: ({ tenantId, tenantSlug, locale, searchParams }) => (
    <ForumSection
      tenantId={tenantId}
      tenantSlug={tenantSlug}
      locale={locale}
      initialCategoryId={typeof searchParams.category === 'string' ? searchParams.category : null}
      initialTopicId={typeof searchParams.topic === 'string' ? searchParams.topic : null}
      initialView={searchParams.view === 'categories' ? 'categories' : 'topics'}
    />
  ),
});
