import { storefrontGraphql } from '@/shared/lib/graphql';
import { readReactionSnapshot, type ReactionSubject } from '../utils/reactions';
export interface ReactionContext { token?: string; tenant?: string; }
const SNAPSHOT = `query BlogReactionSnapshot($subject: ReactionSubjectInput!) {
  reactionSnapshot(subject: $subject) {
    catalog { keys selectionMode maxSelected }
    actorState { revision selected }
    aggregates { reaction count }
  }
}`;
const APPLY = `mutation BlogApplyReaction($input: ApplyReactionInput!) { applyReaction(input: $input) { commandId changed } }`;
export async function loadReactionSnapshot(context: ReactionContext, subject: ReactionSubject) {
  const response = await storefrontGraphql<{ reactionSnapshot: unknown }, { subject: ReactionSubject }>({ query: SNAPSHOT, variables: { subject }, ...context });
  if (response.errors?.length || !response.data?.reactionSnapshot) throw new Error('Reaction snapshot unavailable');
  return readReactionSnapshot(response.data.reactionSnapshot);
}
export async function applyReaction(context: ReactionContext, subject: ReactionSubject, reaction: string, selected: boolean) {
  const input = { commandId: crypto.randomUUID(), subject, reaction, action: selected ? 'REMOVE' : 'ADD' };
  const response = await storefrontGraphql<{ applyReaction: { commandId: string; changed: boolean } }, { input: typeof input }>({ query: APPLY, variables: { input }, ...context });
  if (response.errors?.length || !response.data?.applyReaction) throw new Error('Reaction update unavailable');
  return response.data.applyReaction;
}
