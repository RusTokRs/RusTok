'use client';

import * as React from 'react';
import { Button } from '@/shared/ui/shadcn/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger
} from '@/shared/ui/shadcn/dropdown-menu';
import { MoreHorizontal, ExternalLink, Eye, Copy } from 'lucide-react';
import Link from 'next/link';
import type { OrderListItem } from '../../types';

interface CellActionProps {
  data: OrderListItem;
}

export const CellAction: React.FC<CellActionProps> = ({ data }) => {
  const handleCopyId = React.useCallback(() => {
    if (typeof navigator !== 'undefined') {
      navigator.clipboard.writeText(data.id);
    }
  }, [data.id]);

  return (
    <DropdownMenu modal={false}>
      <DropdownMenuTrigger asChild>
        <Button variant='ghost' className='h-8 w-8 p-0'>
          <span className='sr-only'>Open menu</span>
          <MoreHorizontal className='h-4 w-4' />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align='end'>
        <DropdownMenuLabel>Actions</DropdownMenuLabel>
        <DropdownMenuItem onClick={handleCopyId}>
          <Copy className='mr-2 h-4 w-4' /> Copy Order ID
        </DropdownMenuItem>
        <DropdownMenuItem asChild>
          <Link href={`/dashboard/commerce/orders/${data.id}`}>
            <Eye className='mr-2 h-4 w-4' /> View Details
          </Link>
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem asChild>
          <Link href={`/dashboard/commerce/orders/${data.id}`} target='_blank'>
            <ExternalLink className='mr-2 h-4 w-4' /> Open in new tab
          </Link>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
};
