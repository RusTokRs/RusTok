import { RICH_TEXT_PROFILES } from './generated/profiles';
import type {
  RichTextProfileId,
  RichTextProfileManifest
} from './generated/contracts';

const profiles = new Map<RichTextProfileId, RichTextProfileManifest>(
  RICH_TEXT_PROFILES.map((profile) => [profile.id, profile])
);

export function getRichTextProfile(
  id: RichTextProfileId
): RichTextProfileManifest {
  const profile = profiles.get(id);
  if (!profile) throw new Error(`Unknown richtext profile: ${id}`);
  return profile;
}

export function isRichTextProfileId(value: unknown): value is RichTextProfileId {
  return typeof value === 'string' && profiles.has(value as RichTextProfileId);
}

export { RICH_TEXT_PROFILES };
