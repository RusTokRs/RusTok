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
import { MoreHorizontal, ExternalLink, Edit } from 'lucide-react';
import Link from 'next/link';
import type { ProductListItem } from '../../../api/types';

interface CellActionProps {
  data: ProductListItem;
}

export const CellAction: React.FC<CellActionProps> = ({ data }) => {
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
        <DropdownMenuItem asChild>
          <Link href={`/dashboard/product/${data.id}`}>
            <Edit className='mr-2 h-4 w-4' /> Edit Details
          </Link>
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem asChild>
          <Link href={`/dashboard/product/${data.id}`} target='_blank'>
            <ExternalLink className='mr-2 h-4 w-4' /> Open in new tab
          </Link>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
};
