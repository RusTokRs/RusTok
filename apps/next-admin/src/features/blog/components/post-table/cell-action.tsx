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
  IconWorldOff
} from '@tabler/icons-react';
import { useRouter } from 'next/navigation';
import { useState } from 'react';
import { toast } from 'sonner';
import type { PostSummary, GqlOpts } from '../../api/posts';
import { deletePost, publishPost, unpublishPost } from '../../api/posts';

interface CellActionProps {
  data: PostSummary;
  gqlOpts?: GqlOpts;
}

export const CellAction: React.FC<CellActionProps> = ({
  data,
  gqlOpts = {}
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
    try {
      if (data.status === 'PUBLISHED') {
        await unpublishPost(data.id, gqlOpts);
        toast.success('Post unpublished');
      } else {
        await publishPost(data.id, gqlOpts);
        toast.success('Post published');
      }
      router.refresh();
    } catch {
      toast.error('Failed to change post status');
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
          <Button variant='ghost' className='h-8 w-8 p-0'>
            <span className='sr-only'>Open menu</span>
            <IconDotsVertical className='h-4 w-4' />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align='end'>
          <DropdownMenuLabel>Actions</DropdownMenuLabel>
          <DropdownMenuItem
            onClick={() => router.push(`/dashboard/blog/${data.id}/edit`)}
          >
            <IconEdit className='mr-2 h-4 w-4' /> Edit
          </DropdownMenuItem>
          <DropdownMenuItem onClick={onTogglePublish}>
            {data.status === 'PUBLISHED' ? (
              <>
                <IconWorldOff className='mr-2 h-4 w-4' /> Unpublish
              </>
            ) : (
              <>
                <IconWorldUpload className='mr-2 h-4 w-4' /> Publish
              </>
            )}
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem onClick={() => setOpen(true)}>
            <IconTrash className='mr-2 h-4 w-4' /> Delete
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </>
  );
};
