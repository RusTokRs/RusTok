'use client';

import React, { useState, useEffect, useCallback } from 'react';
import {
  fetchStorefrontCategories,
  fetchStorefrontTopics,
  fetchStorefrontTopic,
  fetchStorefrontReplies,
  fetchForumMemberCards,
  markForumTopicRead,
  type ForumCategoryListItem,
  type ForumTopicListItem,
  type ForumTopicDetail,
  type ForumReplyDetail,
  type ForumMemberCard,
} from '../api/forum';
import { ComposerProvider } from '../context/composer-context';
import { MemberCardProvider } from './member-card';
import { CategoryRail } from './category-rail';
import { TopicFeed } from './topic-feed';
import { ThreadPanel } from './thread-panel';
import { Composer } from './composer';
import { QuoteTooltip } from './quote-tooltip';
import { CategoryOverview } from './category-overview';

interface ForumSectionProps {
  tenantId?: string | null;
  tenantSlug?: string | null;
  locale: string;
  initialCategoryId?: string | null;
  initialTopicId?: string | null;
  initialView?: 'topics' | 'categories';
}

export function ForumSection({
  tenantId,
  tenantSlug,
  locale,
  initialCategoryId = null,
  initialTopicId = null,
  initialView = 'topics',
}: ForumSectionProps) {
  const [activeView, setActiveView] = useState<'topics' | 'categories'>(initialView);
  const [selectedCategoryId, setSelectedCategoryId] = useState<string | null>(initialCategoryId);
  const [selectedTopicId, setSelectedTopicId] = useState<string | null>(initialTopicId);

  const [categories, setCategories] = useState<ForumCategoryListItem[]>([]);
  const [categoriesTotal, setCategoriesTotal] = useState(0);

  const [topics, setTopics] = useState<ForumTopicListItem[]>([]);
  const [topicsTotal, setTopicsTotal] = useState(0);

  const [selectedTopic, setSelectedTopic] = useState<ForumTopicDetail | null>(null);
  const [replies, setReplies] = useState<ForumReplyDetail[]>([]);
  const [repliesTotal, setRepliesTotal] = useState(0);

  const [memberCards, setMemberCards] = useState<ForumMemberCard[]>([]);
  const [isMarkingRead, setIsMarkingRead] = useState(false);
  const [loading, setLoading] = useState(true);

  // Load Categories
  const loadCategories = useCallback(async () => {
    try {
      const data = await fetchStorefrontCategories({
        tenantId: tenantId ?? undefined,
        tenantSlug: tenantSlug ?? undefined,
        locale,
      });
      setCategories(data.items);
      setCategoriesTotal(data.total);
    } catch {
      // Ignore load error
    }
  }, [tenantId, tenantSlug, locale]);

  // Load Topics
  const loadTopics = useCallback(async () => {
    try {
      const data = await fetchStorefrontTopics({
        tenantId: tenantId ?? undefined,
        tenantSlug: tenantSlug ?? undefined,
        categoryId: selectedCategoryId ?? undefined,
        locale,
      });
      setTopics(data.items);
      setTopicsTotal(data.total);

      // Collect author IDs for member cards
      const userIds = Array.from(
        new Set(
          data.items
            .map((t) => t.authorId)
            .filter((id): id is string => Boolean(id))
        )
      );
      if (userIds.length > 0) {
        const cards = await fetchForumMemberCards({
          userIds,
          locale,
          tenantSlug: tenantSlug ?? undefined,
        });
        setMemberCards((prev) => {
          const map = new Map(prev.map((c) => [c.userId, c]));
          for (const c of cards) map.set(c.userId, c);
          return Array.from(map.values());
        });
      }
    } catch {
      // Ignore
    }
  }, [tenantId, tenantSlug, selectedCategoryId, locale]);

  // Load Selected Topic & Replies
  const loadTopicDetails = useCallback(async (topicId: string) => {
    try {
      const [topicData, repliesData] = await Promise.all([
        fetchStorefrontTopic({
          tenantId: tenantId ?? undefined,
          tenantSlug: tenantSlug ?? undefined,
          topicId,
          locale,
        }),
        fetchStorefrontReplies({
          tenantId: tenantId ?? undefined,
          tenantSlug: tenantSlug ?? undefined,
          topicId,
          locale,
        }),
      ]);

      setSelectedTopic(topicData);
      setReplies(repliesData.items);
      setRepliesTotal(repliesData.total);

      // Collect author IDs
      const userIds = Array.from(
        new Set(
          [topicData?.authorId, ...repliesData.items.map((r) => r.authorId)].filter(
            (id): id is string => Boolean(id)
          )
        )
      );
      if (userIds.length > 0) {
        const cards = await fetchForumMemberCards({
          userIds,
          locale,
          tenantSlug: tenantSlug ?? undefined,
        });
        setMemberCards((prev) => {
          const map = new Map(prev.map((c) => [c.userId, c]));
          for (const c of cards) map.set(c.userId, c);
          return Array.from(map.values());
        });
      }
    } catch {
      // Ignore
    }
  }, [tenantId, tenantSlug, locale]);

  useEffect(() => {
    loadCategories();
  }, [loadCategories]);

  useEffect(() => {
    loadTopics();
  }, [loadTopics]);

  useEffect(() => {
    if (selectedTopicId) {
      loadTopicDetails(selectedTopicId);
    } else {
      setSelectedTopic(null);
      setReplies([]);
      setRepliesTotal(0);
    }
  }, [selectedTopicId, loadTopicDetails]);

  // Mark Topic Read
  const handleMarkTopicRead = useCallback(async (topicId: string) => {
    setIsMarkingRead(true);
    try {
      await markForumTopicRead({
        tenantId: tenantId ?? undefined,
        tenantSlug: tenantSlug ?? undefined,
        topicId,
        locale,
      });
      loadTopics();
    } catch {
      // Ignore
    } finally {
      setIsMarkingRead(false);
    }
  }, [tenantId, tenantSlug, locale, loadTopics]);

  // On Topic or Reply Submitted
  const handleSubmitted = useCallback((targetTopicId: string) => {
    setSelectedTopicId(targetTopicId);
    loadTopics();
    loadTopicDetails(targetTopicId);
  }, [loadTopics, loadTopicDetails]);

  return (
    <ComposerProvider>
      <MemberCardProvider cards={memberCards}>
        <section className="overflow-hidden rounded-[2rem] border border-border bg-gradient-to-br from-card via-card to-muted/35 p-6 sm:p-8 shadow-sm">
          {/* Header & View Switcher */}
          <div className="flex flex-col gap-6 sm:flex-row sm:items-end sm:justify-between">
            <div className="max-w-3xl space-y-3">
              <span className="inline-flex items-center gap-2 rounded-full border border-border bg-background/80 px-3 py-1 text-xs font-medium uppercase tracking-[0.22em] text-muted-foreground">
                <span className="h-2 w-2 rounded-full bg-amber-500" />
                Forum
              </span>
              <h2 className="text-3xl font-semibold text-card-foreground">
                Community threads from the module package
              </h2>
              <p className="text-sm leading-6 text-muted-foreground">
                A NodeBB-inspired storefront surface that reads categories, topic feed, and thread replies through the forum module&apos;s public GraphQL contract.
              </p>
            </div>

            {/* View Mode Switcher (Topics vs Categories) */}
            <div className="flex items-center rounded-2xl border border-border bg-background p-1 shadow-xs">
              <button
                type="button"
                onClick={() => setActiveView('topics')}
                className={`flex items-center gap-2 rounded-xl px-4 py-2 text-xs font-semibold transition ${
                  activeView === 'topics'
                    ? 'bg-primary text-primary-foreground shadow-xs'
                    : 'text-muted-foreground hover:text-foreground'
                }`}
              >
                <span>💬</span>
                <span>Discussions</span>
              </button>
              <button
                type="button"
                onClick={() => setActiveView('categories')}
                className={`flex items-center gap-2 rounded-xl px-4 py-2 text-xs font-semibold transition ${
                  activeView === 'categories'
                    ? 'bg-primary text-primary-foreground shadow-xs'
                    : 'text-muted-foreground hover:text-foreground'
                }`}
              >
                <span>📁</span>
                <span>Categories</span>
              </button>
            </div>
          </div>

          {/* Body: Categories Overview vs 3-Column Feed */}
          {activeView === 'categories' ? (
            <div className="mt-8">
              <CategoryOverview
                categories={categories}
                total={categoriesTotal}
                onSelectCategory={(cId) => {
                  setSelectedCategoryId(cId);
                  setSelectedTopicId(null);
                  setActiveView('topics');
                }}
              />
            </div>
          ) : (
            <div className="mt-8 grid gap-6 xl:grid-cols-[16rem_minmax(0,1fr)_24rem]">
              <CategoryRail
                categories={categories}
                total={categoriesTotal}
                selectedCategoryId={selectedCategoryId}
                onSelectCategory={(cId) => {
                  setSelectedCategoryId(cId);
                  setSelectedTopicId(null);
                }}
                onSwitchToOverview={() => setActiveView('categories')}
              />

              <TopicFeed
                topics={topics}
                total={topicsTotal}
                selectedTopicId={selectedTopicId}
                onSelectTopic={(tId) => setSelectedTopicId(tId)}
                selectedCategoryId={selectedCategoryId}
              />

              <ThreadPanel
                topic={selectedTopic}
                replies={replies}
                repliesTotal={repliesTotal}
                onMarkRead={handleMarkTopicRead}
                isMarkingRead={isMarkingRead}
              />
            </div>
          )}

          {/* Global Composer and Floating Tooltip */}
          <Composer
            tenantId={tenantId ?? undefined}
            tenantSlug={tenantSlug ?? undefined}
            locale={locale}
            categories={categories}
            onSubmitted={handleSubmitted}
          />
          <QuoteTooltip />
        </section>
      </MemberCardProvider>
    </ComposerProvider>
  );
}
