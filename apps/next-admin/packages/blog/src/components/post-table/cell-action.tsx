'use client';

import { useTranslations } from '@rustok/next-fluent/client';
import { AlertModal } from '@/widgets/alert-modal';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger
} from '@/components/ui/dropdown-menu';
import {
  IconDotsVertical,
  IconEdit,
  IconEye,
  IconTrash,
  IconWorldUpload,
  IconWorldOff,
  IconArchive,
  IconRestore,
  IconPinned,
  IconPinnedOff
} from '@tabler/icons-react';
import { useRouter } from 'next/navigation';
import { useState } from 'react';
import { toast } from 'sonner';
import type { PostSummary, GqlOpts } from '../../api/posts';
import {
  archivePost,
  deletePost,
  pinPost,
  publishPost,
  restorePost,
  unpinPost,
  unpublishPost
} from '../../api/posts';
import { PreviewTokenModal } from './preview-token-modal';

interface CellActionProps {
  data: PostSummary;
  gqlOpts: GqlOpts;
}

export const CellAction: React.FC<CellActionProps> = ({ data, gqlOpts }) => {
  const t = useTranslations('blog');
  const [loading, setLoading] = useState(false);
  const [open, setOpen] = useState(false);
  const [previewOpen, setPreviewOpen] = useState(false);
  const router = useRouter();

  const onDelete = async () => {
    try {
      setLoading(true);
      await deletePost(data.id, gqlOpts);
      toast.success(t('table.deleted'));
      router.refresh();
    } catch {
      toast.error(t('table.deleteError'));
    } finally {
      setLoading(false);
      setOpen(false);
    }
  };

  const onTogglePublish = async () => {
    if (loading) return;
    setLoading(true);
    try {
      if (data.status === 'ARCHIVED') {
        await restorePost(data.id, gqlOpts);
        toast.success(t('table.restored'));
      } else if (data.status === 'PUBLISHED') {
        await unpublishPost(data.id, gqlOpts);
        toast.success(t('table.unpublished'));
      } else {
        await publishPost(data.id, gqlOpts);
        toast.success(t('table.published'));
      }
      router.refresh();
    } catch {
      toast.error(t('table.statusError'));
    } finally {
      setLoading(false);
    }
  };

  const onArchive = async () => {
    if (loading) return;
    setLoading(true);
    try {
      await archivePost(data.id, gqlOpts);
      toast.success(t('table.archived'));
      router.refresh();
    } catch {
      toast.error(t('table.archiveError'));
    } finally {
      setLoading(false);
    }
  };

  const onTogglePin = async () => {
    if (loading) return;
    setLoading(true);
    try {
      if (data.isPinned) {
        await unpinPost(data.id, gqlOpts);
        toast.success(t('table.unpinned'));
      } else {
        await pinPost(data.id, gqlOpts);
        toast.success(t('table.pinned'));
      }
      router.refresh();
    } catch {
      toast.error(t('table.pinError'));
    } finally {
      setLoading(false);
    }
  };

  return (
    <>
      <AlertModal
        isOpen={open}
        onClose={() => setOpen(false)}
        onConfirm={onDelete}
        loading={loading}
      />
      <PreviewTokenModal
        isOpen={previewOpen}
        onClose={() => setPreviewOpen(false)}
        postId={data.id}
        postTitle={data.title}
        opts={gqlOpts}
      />
      <DropdownMenu modal={false}>
        <DropdownMenuTrigger asChild>
          <Button variant='ghost' className='h-8 w-8 p-0' disabled={loading}>
            <span className='sr-only'>{t('table.openMenu')}</span>
            <IconDotsVertical className='h-4 w-4' />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align='end'>
          <DropdownMenuLabel>{t('table.actions')}</DropdownMenuLabel>
          <DropdownMenuItem
            disabled={loading}
            onClick={() => router.push(`/dashboard/blog/${data.id}/edit`)}
          >
            <IconEdit className='mr-2 h-4 w-4' />
            {t('table.edit')}
          </DropdownMenuItem>
          <DropdownMenuItem disabled={loading} onClick={onTogglePublish}>
            {data.status === 'ARCHIVED' ? (
              <>
                <IconRestore className='mr-2 h-4 w-4' />
                {t('table.restore')}
              </>
            ) : data.status === 'PUBLISHED' ? (
              <>
                <IconWorldOff className='mr-2 h-4 w-4' />
                {t('table.unpublish')}
              </>
            ) : (
              <>
                <IconWorldUpload className='mr-2 h-4 w-4' />
                {t('table.publish')}
              </>
            )}
          </DropdownMenuItem>
          {data.status === 'PUBLISHED' && (
            <DropdownMenuItem disabled={loading} onClick={onArchive}>
              <IconArchive className='mr-2 h-4 w-4' />
              {t('table.archive')}
            </DropdownMenuItem>
          )}
          {(data.status === 'PUBLISHED' || data.isPinned) && (
            <DropdownMenuItem disabled={loading} onClick={onTogglePin}>
              {data.isPinned ? (
                <>
                  <IconPinnedOff className='mr-2 h-4 w-4' />
                  {t('table.unpin')}
                </>
              ) : (
                <>
                  <IconPinned className='mr-2 h-4 w-4' />
                  {t('table.pin')}
                </>
              )}
            </DropdownMenuItem>
          )}
          {data.status === 'DRAFT' && (
            <DropdownMenuItem
              disabled={loading}
              onClick={() => setPreviewOpen(true)}
            >
              <IconEye className='mr-2 h-4 w-4' />
              {t('preview.title')}
            </DropdownMenuItem>
          )}
          <DropdownMenuSeparator />
          <DropdownMenuItem disabled={loading} onClick={() => setOpen(true)}>
            <IconTrash className='mr-2 h-4 w-4' />
            {t('table.delete')}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </>
  );
};
