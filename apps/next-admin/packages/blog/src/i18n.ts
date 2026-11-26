import type { AppMessageArgs, AppMessageKey } from '@/types/i18n';

type ScopedBlogKey = Extract<AppMessageKey, `blog.${string}`>;
type RemoveBlogScope<Key extends ScopedBlogKey> =
  Key extends `blog.${infer Message}` ? Message : never;

export type BlogMessageKey = RemoveBlogScope<ScopedBlogKey>;
export type BlogMessageArgs = {
  [Key in Extract<keyof AppMessageArgs, ScopedBlogKey> as RemoveBlogScope<Key>]:
    AppMessageArgs[Key];
};
