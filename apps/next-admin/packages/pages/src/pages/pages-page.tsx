import type { GqlOpts } from '../api/pages';
import {
  builderUrlForPage,
  listPages,
  storefrontUrlForPage
} from '../api/pages';
import { PageActions } from '../components/page-actions';

interface PagesPageProps {
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
  /** Origin of the Rust (FFA/Leptos) admin that owns the visual builder UI. */
  builderBaseUrl?: string | null;
  /** Storefront base URL for live storefront preview links. */
  storefrontBaseUrl?: string | null;
  /** Secret token to activate Next.js storefront draft mode. */
  previewSecret?: string | null;
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
  storefrontBaseUrl,
  previewSecret,
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
        <p role='alert'>Pages could not be loaded: {loadError}</p>
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
              <th>Storefront</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {items.map((page) => (
              <tr key={page.id}>
                <td>{page.title ?? page.slug ?? page.id}</td>
                <td>{page.slug ?? '—'}</td>
                <td>
                  <span
                    className={
                      page.status === 'published'
                        ? 'inline-flex items-center rounded-full bg-emerald-100 px-2 py-0.5 text-xs font-medium text-emerald-800'
                        : page.status === 'draft'
                          ? 'inline-flex items-center rounded-full bg-amber-100 px-2 py-0.5 text-xs font-medium text-amber-800'
                          : 'inline-flex items-center rounded-full bg-zinc-100 px-2 py-0.5 text-xs font-medium text-zinc-800'
                    }
                  >
                    {page.status}
                  </span>
                </td>
                <td>{page.template}</td>
                <td>{page.updatedAt}</td>
                <td>
                  {builderBaseUrl ? (
                    <a
                      href={builderUrlForPage(builderBaseUrl, page.id)}
                      target='_blank'
                      rel='noreferrer'
                    >
                      Open in builder
                    </a>
                  ) : (
                    '—'
                  )}
                </td>
                <td>
                  {storefrontBaseUrl && page.slug ? (
                    <a
                      href={storefrontUrlForPage(
                        storefrontBaseUrl,
                        page.slug,
                        locale,
                        page.status,
                        previewSecret ?? undefined
                      )}
                      target='_blank'
                      rel='noreferrer'
                    >
                      {page.status === 'published'
                        ? 'View live'
                        : 'Preview draft'}
                    </a>
                  ) : (
                    '—'
                  )}
                </td>
                <td>
                  <PageActions
                    opts={opts}
                    pageId={page.id}
                    status={page.status}
                  />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <p>
        Publishing uses the builder&apos;s reviewed publish flow and is not
        offered here on purpose.
      </p>
    </section>
  );
}
