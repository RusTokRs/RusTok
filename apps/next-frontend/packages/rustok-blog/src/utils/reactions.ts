/** Owner wire values remain decimal strings; never round u64 counters through JS Number. */
export interface ReactionSubject {
  source: 'blog'; kind: 'post'; subjectId: string; subjectRevision: string;
}
export interface ReactionSnapshot {
  catalog: { keys: string[]; selectionMode: 'SINGLE' | 'MULTIPLE'; maxSelected: number };
  actorState: { revision: string; selected: string[] } | null;
  aggregates: Array<{ reaction: string; count: string }>;
}
const MAX_U64 = 18446744073709551615n;
export function decimalU64(value: unknown, positive = false): string {
  if (typeof value !== 'string' || !/^(0|[1-9][0-9]*)$/.test(value) || value.length > 20 || BigInt(value) > MAX_U64 || (positive && value === '0')) {
    throw new Error('Invalid reaction owner value');
  }
  return value;
}
export function blogReactionSubject(postId: string, version: number): ReactionSubject {
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(postId) || /^0{8}-0{4}-0{4}-0{4}-0{12}$/.test(postId) || !Number.isSafeInteger(version) || version < 1 || version > 2147483647) {
    throw new Error('Blog reaction subject requires its exact owner revision');
  }
  return { source: 'blog', kind: 'post', subjectId: postId, subjectRevision: String(version) };
}
export function readReactionSnapshot(value: unknown): ReactionSnapshot {
  const snapshot = value as ReactionSnapshot;
  if (!snapshot?.catalog || !Array.isArray(snapshot.catalog.keys) || snapshot.catalog.keys.some(key => typeof key !== 'string' || !/^[a-z][a-z0-9_]*$/.test(key)) || new Set(snapshot.catalog.keys).size !== snapshot.catalog.keys.length || !['SINGLE', 'MULTIPLE'].includes(snapshot.catalog.selectionMode) || !Number.isInteger(snapshot.catalog.maxSelected) || snapshot.catalog.maxSelected < 1 || !Array.isArray(snapshot.aggregates)) throw new Error('Invalid reaction snapshot');
  const counts = new Set<string>();
  for (const aggregate of snapshot.aggregates) {
    if (!snapshot.catalog.keys.includes(aggregate.reaction) || counts.has(aggregate.reaction)) throw new Error('Invalid reaction aggregate');
    decimalU64(aggregate.count); counts.add(aggregate.reaction);
  }
  if (snapshot.actorState !== null) {
    if (!snapshot.actorState || !Array.isArray(snapshot.actorState.selected) || snapshot.actorState.selected.some(key => !snapshot.catalog.keys.includes(key)) || new Set(snapshot.actorState.selected).size !== snapshot.actorState.selected.length) throw new Error('Invalid reaction actor state');
    decimalU64(snapshot.actorState.revision);
  }
  return snapshot;
}
export function reactionControls(snapshot: ReactionSnapshot) {
  return snapshot.catalog.keys.map(reaction => ({ reaction,
    selected: snapshot.actorState?.selected.includes(reaction) ?? false,
    count: snapshot.aggregates.find(item => item.reaction === reaction)?.count ?? '0',
    canMutate: snapshot.actorState !== null,
  }));
}

/** A mutation timeout may have committed; canonical read follows every outcome. */
export async function settleReactionMutation(
  write: () => Promise<unknown>,
  reload: () => Promise<ReactionSnapshot>,
  isCurrent: () => boolean,
) {
  if (!isCurrent()) return null;
  let writeFailed = false;
  try { await write(); } catch { writeFailed = true; }
  if (!isCurrent()) return null;
  const snapshot = await reload();
  return isCurrent() ? { writeFailed, snapshot } : null;
}
