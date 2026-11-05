'use client';

import { useLocale, useTranslations } from '@rustok/next-fluent';
import { useEffect, useState } from 'react';
import { Copy, ExternalLink } from 'lucide-react';
import { toast } from 'sonner';
import { Modal } from '@/shared/ui/shadcn/modal';
import { Button } from '@/components/ui/button';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import { createPreviewToken, type BlogPreviewToken, type GqlOpts } from '../../api/posts';

/**
 * Draft preview link mint dialog.
 *
 * `createPreviewToken` returns the plaintext secret exactly once — the backend
 * stores only a SHA-256 digest — so the dialog keeps the generated result in
 * local state until it closes. The token is deliberately delivered as a URL
 * *fragment* (`#token=…`): fragments are never sent to the server, keeping the
 * secret out of request logs, proxies and referrers.
 *
 * The storefront origin is not known to this admin host in every deployment, so
 * the absolute link only appears when `NEXT_PUBLIC_STOREFRONT_URL` (or the
 * server-side `STOREFRONT_URL`) is configured; otherwise the editor copies the
 * path and prepends the storefront origin manually.
 */

const TTL_PRESETS = [
  { hours: 24, label: '24 hours' },
  { hours: 72, label: '3 days' },
  { hours: 168, label: '7 days (default)' },
  { hours: 720, label: '30 days' }
] as const;

interface PreviewTokenModalProps {
  isOpen: boolean;
  onClose: () => void;
  postId: string;
  postTitle?: string;
  opts: GqlOpts;
}

function storefrontBaseUrl(): string | null {
  // Client component: only NEXT_PUBLIC_* variables are inlined into the
  // browser bundle, so the storefront origin must come from the public
  // variable (documented in env.example.txt).
  const url = process.env.NEXT_PUBLIC_STOREFRONT_URL ?? null;
  return url ? url.replace(/\/+$/, '') : null;
}

async function copyToClipboard(value: string, successMessage: string, failureMessage: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(value);
    toast.success(successMessage);
  } catch {
    toast.error(failureMessage);
  }
}

export function PreviewTokenModal({
  isOpen,
  onClose,
  postId,
  postTitle,
  opts
}: PreviewTokenModalProps) {
  const t = useTranslations('blog');
  const hostLocale = useLocale();
  const [ttlHours, setTtlHours] = useState<string>('168');
  const [loading, setLoading] = useState(false);
  const [token, setToken] = useState<BlogPreviewToken | null>(null);

  useEffect(() => {
    if (isOpen) {
      return;
    }
    // Drop the secret from component state as soon as the dialog closes; it
    // cannot be fetched again from the backend.
    setToken(null);
    setTtlHours('168');
  }, [isOpen]);

  const onGenerate = async () => {
    if (loading) return;
    setLoading(true);
    try {
      const hours = Number.parseInt(ttlHours, 10);
      const minted = await createPreviewToken(
        { postId, ttlHours: Number.isFinite(hours) ? hours : undefined },
        opts
      );
      setToken(minted);
    } catch {
      toast.error(t('preview.mintError'));
    } finally {
      setLoading(false);
    }
  };

  const previewPath = token ? `/blog/preview#token=${token.token}` : null;
  const baseUrl = storefrontBaseUrl();
  const absoluteUrl = previewPath && baseUrl ? `${baseUrl}${previewPath}` : null;
  const expiresLabel = token
    ? new Date(token.expiresAt).toLocaleString(hostLocale, {
        dateStyle: 'medium',
        timeStyle: 'short'
      })
    : null;

  return (
    <Modal
      title={t('preview.title')}
      description={
        token
          ? t('preview.descriptionReady')
          : t('preview.description')
      }
      isOpen={isOpen}
      onClose={onClose}
    >
      {!token ? (
        <div className='space-y-4 pt-2'>
          {postTitle ? (
            <p className='text-sm text-muted-foreground'>{t('preview.post')}<span className='font-medium text-foreground'>{postTitle}</span>
            </p>
          ) : null}
          <div className='space-y-2'>
            <label className='text-sm font-medium' htmlFor='preview-token-ttl'>{t('preview.lifetime')}</label>
            <Select value={ttlHours} onValueChange={setTtlHours}>
              <SelectTrigger id='preview-token-ttl' className='w-full'>
                <SelectValue placeholder={t('preview.lifetime')} />
              </SelectTrigger>
              <SelectContent>
                {TTL_PRESETS.map((preset) => (
                  <SelectItem key={preset.hours} value={String(preset.hours)}>
                    {t('preview.lifetimeHours', { hours: preset.hours })}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className='flex items-center justify-end space-x-2 pt-2'>
            <Button disabled={loading} variant='outline' onClick={onClose}>{t('categories.cancel')}</Button>
            <Button disabled={loading} onClick={onGenerate}>
              {loading ? 'Generating…' : 'Generate link'}
            </Button>
          </div>
        </div>
      ) : (
        <div className='space-y-4 pt-2'>
          <div className='space-y-1'>
            <p className='text-sm font-medium'>{t('preview.title')}</p>
            <code className='block max-w-full overflow-x-auto rounded bg-muted px-2 py-1.5 text-xs'>
              {absoluteUrl ?? previewPath}
            </code>
            <p className='text-xs text-muted-foreground'>
              {absoluteUrl
                ? t('preview.linkHint')
                : t('preview.pathHint')}
            </p>
          </div>
          <div className='flex flex-wrap items-center gap-2'>
            <Button
              size='sm'
              variant='outline'
              onClick={() =>
                previewPath && copyToClipboard(absoluteUrl ?? previewPath, t('preview.linkCopied'), t('preview.copyError'))
              }
            >
              <Copy className='mr-2 h-4 w-4' />{t('preview.copyLink')}</Button>
            {absoluteUrl ? (
              <Button size='sm' variant='outline' asChild>
                <a href={absoluteUrl} target='_blank' rel='noopener noreferrer'>
                  <ExternalLink className='mr-2 h-4 w-4' />{t('preview.open')}</a>
              </Button>
            ) : null}
            <Button
              size='sm'
              variant='ghost'
              onClick={() => copyToClipboard(token.token, t('preview.tokenCopied'), t('preview.copyError'))}
            >
              <Copy className='mr-2 h-4 w-4' />{t('preview.copyToken')}</Button>
          </div>
          <p className='text-xs text-muted-foreground'>{t('preview.expires', { at: expiresLabel ?? '' })}</p>
          <div className='flex items-center justify-end'>
            <Button variant='outline' onClick={onClose}>{t('preview.done')}</Button>
          </div>
        </div>
      )}
    </Modal>
  );
}
