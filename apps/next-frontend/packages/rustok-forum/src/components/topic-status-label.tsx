'use client';

import { useTranslations } from '@rustok/next-fluent';

/**
 * Badge label for the raw topic status token the Forum owner transport returns.
 *
 * The owner exposes `open`, `closed`, `archived`, and `pending`; the extra tokens keep the
 * badge honest for a status a deployment configures later. An unrecognized token is rendered
 * unchanged instead of being claimed as a known status.
 */
const STATUS_KEYS: Record<string, 'statusOpen' | 'statusClosed' | 'statusArchived' | 'statusPending'> = {
  open: 'statusOpen',
  published: 'statusOpen',
  active: 'statusOpen',
  approved: 'statusOpen',
  closed: 'statusClosed',
  hidden: 'statusClosed',
  archived: 'statusArchived',
  pending: 'statusPending',
  draft: 'statusPending',
};

export function TopicStatusLabel({ status }: { status: string }) {
  const forum = useTranslations('Forum');
  const key = STATUS_KEYS[status.toLowerCase()];
  return <>{key ? forum(key) : status}</>;
}
