import { fetchPublishedPosts, type BlogGraphqlExecutor } from "../api/posts";
import { PostCard } from "./post-card";

export async function BlogSection({ graphql, tenantId, tenantSlug }: {
  graphql: BlogGraphqlExecutor;
  tenantId: string | null;
  tenantSlug: string | null;
}) {
  if (!tenantId) return null;

  let posts;
  try {
    posts = (await fetchPublishedPosts(graphql, tenantId, tenantSlug)).items;
  } catch {
    return null;
  }
  if (posts.length === 0) return null;

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-2xl font-semibold text-foreground">Latest from the Blog</h2>
        <p className="mt-1 text-sm text-muted-foreground">Recent posts and updates</p>
      </div>
      <div className="grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
        {posts.map((post) => <PostCard key={post.id} post={post} />)}
      </div>
    </div>
  );
}
