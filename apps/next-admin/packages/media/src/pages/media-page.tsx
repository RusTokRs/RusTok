import type { GqlOpts, MediaItem } from '../api/media';
import { listMedia, mediaUsage } from '../api/media';
import { MediaActions } from '../components/media-actions';

interface MediaPageProps {
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/**
 * Media library surface for the classic Next admin, consuming the same
 * rustok-media GraphQL as the Rust FFA admin. Uploads stay with the owning
 * modules' flows; this surface lists, inspects, and deletes tenant assets
 * (the same assets page-builder images reference).
 */
export default async function MediaPage({
  token,
  tenantSlug,
  tenantId
}: MediaPageProps) {
  const opts: GqlOpts = { token, tenantSlug, tenantId };
  let items: MediaItem[] = [];
  let usage: { fileCount: number; totalBytes: number } | null = null;
  let loadError: string | null = null;
  try {
    const [list, stats] = await Promise.all([
      listMedia(opts),
      mediaUsage(opts)
    ]);
    items = list.items;
    usage = stats;
  } catch (cause) {
    loadError = cause instanceof Error ? cause.message : 'Failed to load media';
  }

  return (
    <section>
      <h1>Media</h1>
      {usage ? (
        <p>
          {usage.fileCount} files, {formatBytes(usage.totalBytes)} total
        </p>
      ) : null}
      {loadError ? (
        <p role='alert'>Media could not be loaded: {loadError}</p>
      ) : items.length === 0 ? (
        <p>No media assets yet.</p>
      ) : (
        <table>
          <thead>
            <tr>
              <th>File</th>
              <th>Type</th>
              <th>Size</th>
              <th>Dimensions</th>
              <th>Owner module</th>
              <th>Uploaded</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {items.map((item) => (
              <tr key={item.id}>
                <td>
                  <a href={item.publicUrl} target='_blank' rel='noreferrer'>
                    {item.originalName}
                  </a>
                </td>
                <td>{item.mimeType}</td>
                <td>{formatBytes(item.size)}</td>
                <td>
                  {item.width && item.height
                    ? `${item.width}×${item.height}`
                    : '—'}
                </td>
                <td>{item.ownerModule}</td>
                <td>{item.createdAt}</td>
                <td>
                  <MediaActions opts={opts} mediaId={item.id} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
