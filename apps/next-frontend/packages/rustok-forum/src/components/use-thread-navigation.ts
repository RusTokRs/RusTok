'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import {
  findThreadPostIndex,
  type ThreadJump,
  type ThreadPostTarget,
} from './thread-navigation';

/** DOM positioning is scoped to this mounted thread, never the whole forum document. */
export function useThreadNavigation({
  scopeKey,
  replies,
  hasMore,
  loadReplies,
}: {
  scopeKey: string;
  replies: readonly { id: string }[];
  hasMore: boolean;
  loadReplies?: (target: ThreadPostTarget) => Promise<ThreadJump | null>;
}) {
  const loadedReplies = replies.length;
  const postsRef = useRef<HTMLDivElement>(null);
  const scopeRef = useRef(scopeKey);
  scopeRef.current = scopeKey;
  const requestRef = useRef(0);
  const busyRef = useRef(false);
  const appliedJumpRef = useRef<{ key: string; post: number } | null>(null);
  const [position, setPosition] = useState({ key: scopeKey, post: 1 });
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [incompleteKey, setIncompleteKey] = useState<string | null>(null);
  const [jump, setJump] = useState<{ key: string; post: number } | null>(null);

  useEffect(() => {
    busyRef.current = false;
    ++requestRef.current;
    setBusyKey(null);
    setIncompleteKey(null);
    setJump(null);
    return () => {
      ++requestRef.current;
    };
  }, [scopeKey]);

  useEffect(() => {
    const root = postsRef.current;
    if (!root) return;
    const posts = root.querySelectorAll<HTMLElement>(
      '[data-thread-post-number]',
    );
    let frame: number | null = null;
    const measure = () => {
      frame = null;
      const index = findThreadPostIndex(
        posts.length,
        (i) => posts[i].getBoundingClientRect().top,
        window.innerHeight * 0.25,
      );
      if (index >= 0) {
        const post = Number(posts[index].dataset.threadPostNumber);
        setPosition((prior) =>
          prior.key === scopeKey && prior.post === post
            ? prior
            : { key: scopeKey, post },
        );
      }
    };
    const schedule = () => {
      if (frame === null) frame = window.requestAnimationFrame(measure);
    };
    window.addEventListener('scroll', schedule, { passive: true });
    window.addEventListener('resize', schedule);
    const resize = new ResizeObserver(schedule);
    resize.observe(root);
    schedule();
    return () => {
      window.removeEventListener('scroll', schedule);
      window.removeEventListener('resize', schedule);
      resize.disconnect();
      if (frame !== null) window.cancelAnimationFrame(frame);
    };
  }, [scopeKey, replies]);

  // Paging commits first; scrolling before this effect would address the old DOM.
  useEffect(() => {
    if (!jump || jump.key !== scopeKey || appliedJumpRef.current === jump)
      return;
    const target = postsRef.current?.querySelector<HTMLElement>(
      `[data-thread-post-number="${jump.post}"]`,
    );
    if (target) {
      target.scrollIntoView({
        behavior: 'smooth',
        block: jump.post === 1 ? 'start' : 'center',
      });
      appliedJumpRef.current = jump;
    }
  }, [jump, scopeKey, replies]);

  const jumpToPost = useCallback(
    async (target: ThreadPostTarget) => {
      if (busyRef.current) return;
      if (target !== 'last' && (!Number.isSafeInteger(target) || target < 1))
        return;
      const request = ++requestRef.current;
      const isCurrent = () =>
        request === requestRef.current && scopeRef.current === scopeKey;
      busyRef.current = true;
      setBusyKey(scopeKey);
      setIncompleteKey(null);
      try {
        let result: ThreadJump | null;
        if (
          target === 1 ||
          (target !== 'last' && target <= loadedReplies + 1) ||
          !hasMore
        ) {
          result = {
            post:
              target === 'last'
                ? loadedReplies + 1
                : Math.min(target, loadedReplies + 1),
            complete: true,
          };
        } else {
          result = (await loadReplies?.(target)) ?? null;
        }
        if (!isCurrent() || !result) return;
        setIncompleteKey(result.complete ? null : scopeKey);
        setJump({ key: scopeKey, post: result.post });
      } finally {
        if (isCurrent()) {
          busyRef.current = false;
          setBusyKey(null);
        }
      }
    },
    [scopeKey, loadedReplies, hasMore, loadReplies],
  );

  return {
    postsRef,
    currentPost:
      position.key === scopeKey
        ? Math.min(position.post, loadedReplies + 1)
        : 1,
    isNavigating: busyKey === scopeKey,
    navigationIncomplete: incompleteKey === scopeKey && hasMore,
    jumpToPost,
  };
}
