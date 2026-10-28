'use client';

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
import { archivePost, deletePost, pinPost, publishPost, restorePost, unpinPost, unpublishPost } from '../../api/posts';

interface CellActionProps {
  data: PostSummary;
  gqlOpts: GqlOpts;
}

export const CellAction: React.FC<CellActionProps> = ({
  data,
  gqlOpts
}) => {
  const [loading, setLoading] = useState(false);
  const [open, setOpen] = useState(false);
  const router = useRouter();

  const onDelete = async () => {
    try {
      setLoading(true);
      await deletePost(data.id, gqlOpts);
      toast.success('Post deleted');
      router.refresh();
    } catch {
      toast.error('Failed to delete post');
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
        toast.success('Post restored to draft');
      } else if (data.status === 'PUBLISHED') {
        await unpublishPost(data.id, gqlOpts);
        toast.success('Post unpublished');
      } else {
        await publishPost(data.id, gqlOpts);
        toast.success('Post published');
      }
      router.refresh();
    } catch {
      toast.error('Failed to change post status');
    } finally {
      setLoading(false);
    }
  };

  const onArchive = async () => {
    if (loading) return;
    setLoading(true);
    try {
      await archivePost(data.id, gqlOpts);
      toast.success('Post archived');
      router.refresh();
    } catch {
      toast.error('Failed to archive post');
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
        toast.success('Post unpinned');
      } else {
        await pinPost(data.id, gqlOpts);
        toast.success('Post pinned');
      }
      router.refresh();
    } catch {
      toast.error('Failed to change post pin state');
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
      <DropdownMenu modal={false}>
        <DropdownMenuTrigger asChild>
          <Button variant='ghost' className='h-8 w-8 p-0' disabled={loading}>
            <span className='sr-only'>Open menu</span>
            <IconDotsVertical className='h-4 w-4' />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align='end'>
          <DropdownMenuLabel>Actions</DropdownMenuLabel>
          <DropdownMenuItem
            disabled={loading}
            onClick={() => router.push(`/dashboard/blog/${data.id}/edit`)}
          >
            <IconEdit className='mr-2 h-4 w-4' /> Edit
          </DropdownMenuItem>
          <DropdownMenuItem disabled={loading} onClick={onTogglePublish}>
            {data.status === 'ARCHIVED' ? (
              <>
                <IconRestore className='mr-2 h-4 w-4' /> Restore to draft
              </>
            ) : data.status === 'PUBLISHED' ? (
              <>
                <IconWorldOff className='mr-2 h-4 w-4' /> Unpublish
              </>
            ) : (
              <>
                <IconWorldUpload className='mr-2 h-4 w-4' /> Publish
              </>
            )}
          </DropdownMenuItem>
          {data.status === 'PUBLISHED' && (
            <DropdownMenuItem disabled={loading} onClick={onArchive}>
              <IconArchive className='mr-2 h-4 w-4' /> Archive
            </DropdownMenuItem>
          )}
          {(data.status === 'PUBLISHED' || data.isPinned) && (
            <DropdownMenuItem disabled={loading} onClick={onTogglePin}>
              {data.isPinned ? (
                <><IconPinnedOff className='mr-2 h-4 w-4' /> Unpin</>
              ) : (
                <><IconPinned className='mr-2 h-4 w-4' /> Pin</>
              )}
            </DropdownMenuItem>
          )}
          <DropdownMenuSeparator />
          <DropdownMenuItem disabled={loading} onClick={() => setOpen(true)}>
            <IconTrash className='mr-2 h-4 w-4' /> Delete
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </>
  );
};
