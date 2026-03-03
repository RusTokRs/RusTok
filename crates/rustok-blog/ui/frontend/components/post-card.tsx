import type { PublicPostSummary } from '../api/posts';

export function PostCard({ post }: { post: PublicPostSummary }) {
  const date = post.publishedAt
    ? new Date(post.publishedAt).toLocaleDateString()
    : null;

  return (
    <article className='rounded-2xl border border-border bg-card p-5 shadow-sm transition-shadow hover:shadow-md'>
      {post.featuredImageUrl && (
        <img
          src={post.featuredImageUrl}
          alt={post.title}
          className='mb-4 aspect-video w-full rounded-lg object-cover'
        />
      )}
      <h3 className='text-lg font-semibold text-card-foreground'>
        {post.title}
      </h3>
      {post.excerpt && (
        <p className='mt-2 line-clamp-3 text-sm text-muted-foreground'>
          {post.excerpt}
        </p>
      )}
      <div className='mt-3 flex items-center gap-2 text-xs text-muted-foreground'>
        {date && <span>{date}</span>}
      </div>
      {post.tags.length > 0 && (
        <div className='mt-3 flex flex-wrap gap-1.5'>
          {post.tags.map((tag) => (
            <span
              key={tag}
              className='rounded-full bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary'
            >
              {tag}
            </span>
          ))}
        </div>
      )}
    </article>
  );
}
