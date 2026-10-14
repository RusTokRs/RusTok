import type { GqlOpts } from '../api/pages';
import { builderUrlForPage, listPages } from '../api/pages';
import { PageActions } from '../components/page-actions';

interface PagesPageProps {
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
  /** Origin of the Rust (FFA/Leptos) admin that owns the visual builder UI. */
  builderBaseUrl?: string | null;
  locale?: string;
}

/**
 * Pages management surface for the classic Next admin. It consumes exactly the
 * GraphQL the Rust FFA admin's transport consumes; the visual editor remains
 * module-owned UI of `rustok-pages-admin` and is reached through "Open in
 * builder" deep links instead of a second editor implementation.
 */
export default async function PagesPage({
  token,
  tenantSlug,
  tenantId,
  builderBaseUrl,
  locale = 'en'
}: PagesPageProps) {
  const opts: GqlOpts = { token, tenantSlug, tenantId };
  let items: Awaited<ReturnType<typeof listPages>>['items'] = [];
  let loadError: string | null = null;
  try {
    const result = await listPages({ locale, page: 1, perPage: 50 }, opts);
    items = result.items;
  } catch (cause) {
    loadError = cause instanceof Error ? cause.message : 'Failed to load pages';
  }

  return (
    <section>
      <h1>Pages</h1>
      <PageActions opts={opts} locale={locale} />
      {loadError ? (
        <p role="alert">Pages could not be loaded: {loadError}</p>
      ) : items.length === 0 ? (
        <p>No pages yet. Create the first draft above.</p>
      ) : (
        <table>
          <thead>
            <tr>
              <th>Title</th>
              <th>Slug</th>
              <th>Status</th>
              <th>Template</th>
              <th>Updated</th>
              <th>Builder</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {items.map((page) => (
              <tr key={page.id}>
                <td>{page.title ?? page.slug ?? page.id}</td>
                <td>{page.slug ?? '—'}</td>
                <td>{page.status}</td>
                <td>{page.template}</td>
                <td>{page.updatedAt}</td>
                <td>
                  {builderBaseUrl ? (
                    <a
                      href={builderUrlForPage(builderBaseUrl, page.id)}
                      target="_blank"
                      rel="noreferrer"
                    >
                      Open in builder
                    </a>
                  ) : (
                    '—'
                  )}
                </td>
                <td>
                  <PageActions opts={opts} pageId={page.id} status={page.status} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <p>
        Publishing uses the builder&apos;s reviewed publish flow and is not offered here on
        purpose.
      </p>
    </section>
  );
}
