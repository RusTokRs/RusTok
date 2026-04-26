'use client';

import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import type { Editor } from 'grapesjs';
import { Loader2, TriangleAlert } from 'lucide-react';
import { useLocale } from 'next-intl';
import { useEffect, useRef, useState } from 'react';
import { toast } from 'sonner';
import type { GqlOpts } from '../api/posts';
import { updatePageBody, type PageBlock, type PageBody } from '../api/pages';

const GRAPESJS_FORMAT = 'grapesjs_v1';

export function PageBuilder({
  pageId,
  initialBody = null,
  initialBlocks = [],
  initialLocale,
  pageTitle,
  gqlOpts = {}
}: {
  pageId: string;
  initialBody?: PageBody | null;
  initialBlocks?: PageBlock[];
  initialLocale?: string;
  pageTitle?: string | null;
  gqlOpts?: GqlOpts;
}) {
  const hostLocale = useLocale();
  const containerRef = useRef<HTMLDivElement | null>(null);
  const editorRef = useRef<Editor | null>(null);
  const [isReady, setIsReady] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [lastSavedAt, setLastSavedAt] = useState<string | null>(
    initialBody?.updatedAt ?? null
  );

  const locale = initialBody?.locale ?? initialLocale ?? hostLocale;
  const hasLegacyBlocks = initialBlocks.length > 0;
  const isExistingGrapesProject = initialBody?.format === GRAPESJS_FORMAT;
  const initialProjectData =
    isExistingGrapesProject && initialBody?.contentJson
      ? initialBody.contentJson
      : null;

  useEffect(() => {
    let disposed = false;
    let editor: Editor | null = null;

    async function bootstrapEditor() {
      if (!containerRef.current || editorRef.current) {
        return;
      }

      try {
        const [{ default: grapesjs }, { default: presetWebpage }] =
          await Promise.all([
            import('grapesjs'),
            import('grapesjs-preset-webpage')
          ]);

        if (disposed || !containerRef.current) {
          return;
        }

        editor = grapesjs.init({
          container: containerRef.current,
          height: '70vh',
          width: 'auto',
          storageManager: false,
          noticeOnUnload: false,
          fromElement: false,
          plugins: [presetWebpage],
          pluginsOpts: {
            'grapesjs-preset-webpage': {
              modalImportTitle: 'Import project markup',
              modalImportButton: 'Import',
              modalImportLabel: '',
              modalImportContent: ''
            }
          }
        });

        editorRef.current = editor;

        if (initialProjectData) {
          editor.loadProjectData(initialProjectData);
        } else {
          editor.setComponents(buildDefaultMarkup(pageTitle));
          editor.setStyle(defaultCss());
        }

        if (!disposed) {
          setIsReady(true);
          setLoadError(null);
        }
      } catch (error) {
        if (!disposed) {
          setLoadError(
            error instanceof Error
              ? error.message
              : 'Failed to initialize GrapesJS editor'
          );
        }
      }
    }

    void bootstrapEditor();

    return () => {
      disposed = true;
      editorRef.current?.destroy();
      editorRef.current = null;
      editor = null;
    };
  }, [initialProjectData, pageTitle]);

  async function handleSave() {
    if (!editorRef.current) {
      toast.error('Page builder is still loading');
      return;
    }

    setIsSaving(true);

    try {
      const updatedBody = await updatePageBody(
        pageId,
        {
          locale,
          format: GRAPESJS_FORMAT,
          content: '',
          contentJson: editorRef.current.getProjectData() as Record<
            string,
            unknown
          >
        },
        gqlOpts
      );

      setLastSavedAt(updatedBody.updatedAt);
      toast.success('Page project saved');
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : 'Failed to save page project'
      );
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <Card>
      <CardHeader className='space-y-3'>
        <div className='flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between'>
          <div className='space-y-1'>
            <CardTitle>Visual Page Builder</CardTitle>
            <p className='text-muted-foreground text-sm'>
              GrapesJS project data is stored in the page body as `grapesjs_v1`.
            </p>
          </div>
          <div className='flex flex-wrap items-center gap-2'>
            {lastSavedAt ? (
              <span className='text-muted-foreground text-xs'>
                Last saved: {new Date(lastSavedAt).toLocaleString()}
              </span>
            ) : null}
            <Button
              type='button'
              disabled={!isReady || isSaving}
              onClick={() => void handleSave()}
            >
              {isSaving ? (
                <Loader2 className='mr-2 size-4 animate-spin' />
              ) : null}
              Save project
            </Button>
          </div>
        </div>

        {!isExistingGrapesProject || hasLegacyBlocks ? (
          <div className='rounded-md border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-950 dark:border-amber-900 dark:bg-amber-950/40 dark:text-amber-100'>
            <div className='flex items-start gap-2'>
              <TriangleAlert className='mt-0.5 size-4 shrink-0' />
              <div className='space-y-1'>
                {!isExistingGrapesProject ? (
                  <p>
                    This page does not have a saved GrapesJS project yet. The
                    first save will switch its body format to `grapesjs_v1`.
                  </p>
                ) : null}
                {hasLegacyBlocks ? (
                  <p>
                    Legacy block payload is still attached to this page (
                    {initialBlocks.length} blocks). It is left untouched for
                    now, so storefront migration can happen safely and
                    explicitly.
                  </p>
                ) : null}
              </div>
            </div>
          </div>
        ) : null}
      </CardHeader>

      <CardContent className='space-y-4'>
        {loadError ? (
          <div className='border-destructive/30 bg-destructive/10 text-destructive rounded-md border px-4 py-3 text-sm'>
            {loadError}
          </div>
        ) : null}

        {!isReady && !loadError ? (
          <div className='text-muted-foreground flex min-h-24 items-center gap-2 rounded-md border border-dashed px-4 py-3 text-sm'>
            <Loader2 className='size-4 animate-spin' />
            Loading GrapesJS editor...
          </div>
        ) : null}

        <div className='rustok-grapesjs overflow-hidden rounded-lg border'>
          <div ref={containerRef} />
        </div>
      </CardContent>
    </Card>
  );
}

function buildDefaultMarkup(pageTitle?: string | null): string {
  const title = escapeHtml(pageTitle?.trim() || 'New page');

  return `
    <main class="rtk-page-shell">
      <section class="rtk-hero">
        <div class="rtk-container">
          <span class="rtk-kicker">RusToK Pages</span>
          <h1>${title}</h1>
          <p>Start building this page visually with GrapesJS. Replace this initial section with the page layout, content, and calls to action for this tenant.</p>
          <div class="rtk-actions">
            <a class="rtk-button rtk-button--primary" href="#content">Explore</a>
            <a class="rtk-button rtk-button--secondary" href="#contact">Contact us</a>
          </div>
        </div>
      </section>
      <section id="content" class="rtk-section">
        <div class="rtk-container rtk-grid">
          <article class="rtk-card">
            <h2>Flexible sections</h2>
            <p>Use blocks, media, forms, and layout tools from GrapesJS to shape the page structure.</p>
          </article>
          <article class="rtk-card">
            <h2>Reusable project data</h2>
            <p>The builder stores canonical project JSON, so both Next and Leptos can hydrate the same page definition.</p>
          </article>
          <article id="contact" class="rtk-card">
            <h2>Publishing flow</h2>
            <p>Save this draft into the page body first, then wire storefront rendering in the next rollout slice.</p>
          </article>
        </div>
      </section>
    </main>
  `;
}

function defaultCss(): string {
  return `
    * { box-sizing: border-box; }
    body {
      margin: 0;
      font-family: "Segoe UI", sans-serif;
      color: #172554;
      background: #fffdf8;
    }
    .rtk-page-shell {
      min-height: 100vh;
      background:
        radial-gradient(circle at top left, rgba(15, 118, 110, 0.18), transparent 28rem),
        radial-gradient(circle at top right, rgba(251, 146, 60, 0.18), transparent 24rem),
        linear-gradient(180deg, #fffdf8 0%, #f8fafc 100%);
    }
    .rtk-container {
      max-width: 72rem;
      margin: 0 auto;
      padding: 0 1.5rem;
    }
    .rtk-hero {
      padding: 7rem 0 5rem;
    }
    .rtk-kicker {
      display: inline-flex;
      align-items: center;
      gap: 0.5rem;
      padding: 0.35rem 0.8rem;
      border-radius: 999px;
      background: rgba(15, 118, 110, 0.12);
      color: #0f766e;
      font-size: 0.85rem;
      font-weight: 700;
      letter-spacing: 0.08em;
      text-transform: uppercase;
    }
    .rtk-hero h1 {
      max-width: 42rem;
      margin: 1.5rem 0 1rem;
      font-size: clamp(2.75rem, 5vw, 4.75rem);
      line-height: 0.98;
    }
    .rtk-hero p {
      max-width: 42rem;
      margin: 0;
      color: #334155;
      font-size: 1.1rem;
      line-height: 1.75;
    }
    .rtk-actions {
      display: flex;
      flex-wrap: wrap;
      gap: 0.75rem;
      margin-top: 2rem;
    }
    .rtk-button {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      padding: 0.85rem 1.2rem;
      border-radius: 999px;
      text-decoration: none;
      font-weight: 600;
      transition: transform 160ms ease, box-shadow 160ms ease;
    }
    .rtk-button:hover {
      transform: translateY(-1px);
    }
    .rtk-button--primary {
      background: #0f766e;
      color: white;
      box-shadow: 0 12px 24px rgba(15, 118, 110, 0.18);
    }
    .rtk-button--secondary {
      border: 1px solid rgba(15, 23, 42, 0.12);
      color: #172554;
      background: rgba(255, 255, 255, 0.72);
    }
    .rtk-section {
      padding: 0 0 5rem;
    }
    .rtk-grid {
      display: grid;
      gap: 1.25rem;
      grid-template-columns: repeat(auto-fit, minmax(16rem, 1fr));
    }
    .rtk-card {
      padding: 1.5rem;
      border-radius: 1.5rem;
      border: 1px solid rgba(148, 163, 184, 0.24);
      background: rgba(255, 255, 255, 0.86);
      box-shadow: 0 18px 48px rgba(15, 23, 42, 0.08);
    }
    .rtk-card h2 {
      margin: 0 0 0.75rem;
      font-size: 1.2rem;
    }
    .rtk-card p {
      margin: 0;
      color: #334155;
      line-height: 1.7;
    }
  `;
}

function escapeHtml(input: string): string {
  return input
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}
