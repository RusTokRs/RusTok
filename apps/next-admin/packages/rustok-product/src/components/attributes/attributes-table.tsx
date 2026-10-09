'use client';

import * as React from 'react';
import { DataTableStatic } from '@/widgets/data-table/data-table-static';
import type { DataTableStaticColumn } from '@/widgets/data-table/data-table-static';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import { Search, Sliders, Check, Minus, PlusCircle } from 'lucide-react';
import type {
  ProductAttributeSummary,
  CreateProductAttributeOptionPayload
} from '../../api/types';
import { AttributeOptionsDialog } from './attribute-options-dialog';

interface AttributesTableProps {
  attributes: ProductAttributeSummary[];
  onCreateOption: (
    payload: CreateProductAttributeOptionPayload
  ) => Promise<void>;
}

/** A boolean attribute flag: a check for true, a dash for false. */
function BooleanFlag({ value }: { value: boolean }) {
  return value ? (
    <Check className='inline h-4 w-4 text-emerald-600' />
  ) : (
    <Minus className='text-muted-foreground/40 inline h-3.5 w-3.5' />
  );
}

export function AttributesTable({
  attributes,
  onCreateOption
}: AttributesTableProps) {
  const [search, setSearch] = React.useState('');
  const [selectedAttribute, setSelectedAttribute] =
    React.useState<ProductAttributeSummary | null>(null);
  const [optionDialogOpen, setOptionDialogOpen] = React.useState(false);

  const filteredAttributes = React.useMemo(() => {
    if (!search.trim()) return attributes;
    const q = search.toLowerCase().trim();
    return attributes.filter(
      (a) =>
        a.label.toLowerCase().includes(q) || a.code.toLowerCase().includes(q)
    );
  }, [attributes, search]);

  const handleOpenAddOption = (attr: ProductAttributeSummary) => {
    setSelectedAttribute(attr);
    setOptionDialogOpen(true);
  };

  const columns: DataTableStaticColumn<ProductAttributeSummary>[] = [
    {
      id: 'label',
      header: 'Label',
      headerClassName: 'w-[240px]',
      cellClassName: 'text-sm font-medium',
      cell: (attr) => attr.label
    },
    {
      id: 'code',
      header: 'Code',
      cell: (attr) => (
        <code className='bg-muted rounded px-1.5 py-0.5 font-mono text-xs'>
          {attr.code}
        </code>
      )
    },
    {
      id: 'valueType',
      header: 'Value Type',
      cell: (attr) => (
        <Badge variant='outline' className='text-xs font-normal capitalize'>
          {attr.valueType.replace('_', ' ')}
        </Badge>
      )
    },
    {
      id: 'isLocalized',
      header: 'Localized',
      headerClassName: 'text-center',
      cellClassName: 'text-center',
      cell: (attr) => <BooleanFlag value={attr.isLocalized} />
    },
    {
      id: 'isFilterable',
      header: 'Filterable',
      headerClassName: 'text-center',
      cellClassName: 'text-center',
      cell: (attr) => <BooleanFlag value={attr.isFilterable} />
    },
    {
      id: 'isSortable',
      header: 'Sortable',
      headerClassName: 'text-center',
      cellClassName: 'text-center',
      cell: (attr) => <BooleanFlag value={attr.isSortable} />
    },
    {
      id: 'showOnStorefront',
      header: 'Storefront',
      headerClassName: 'text-center',
      cellClassName: 'text-center',
      cell: (attr) => <BooleanFlag value={attr.showOnStorefront} />
    },
    {
      id: 'actions',
      header: 'Actions',
      headerClassName: 'text-right',
      cellClassName: 'text-right',
      cell: (attr) => {
        const isOptionType =
          attr.valueType === 'option' || attr.valueType === 'multi_option';
        return isOptionType ? (
          <Button
            variant='ghost'
            size='sm'
            className='h-8 text-xs'
            onClick={() => handleOpenAddOption(attr)}
          >
            <PlusCircle className='mr-1 h-3.5 w-3.5' />
            Add Option
          </Button>
        ) : (
          <span className='text-muted-foreground text-xs'>-</span>
        );
      }
    }
  ];

  return (
    <div className='space-y-4'>
      <div className='flex items-center gap-3'>
        <div className='relative max-w-sm flex-1'>
          <Search className='text-muted-foreground absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2' />
          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder='Filter attributes by label or code...'
            className='pl-9'
          />
        </div>
        <p className='text-muted-foreground ml-auto text-xs'>
          Total attributes:{' '}
          <span className='text-foreground font-semibold'>
            {attributes.length}
          </span>
        </p>
      </div>

      <DataTableStatic
        rows={filteredAttributes}
        columns={columns}
        getRowKey={(attr) => attr.id}
        emptyState={
          <div className='flex flex-col items-center justify-center gap-1.5'>
            <Sliders className='text-muted-foreground/50 h-8 w-8' />
            <p>No product attributes found.</p>
          </div>
        }
      />

      <AttributeOptionsDialog
        attribute={selectedAttribute}
        open={optionDialogOpen}
        onOpenChange={setOptionDialogOpen}
        onCreateOption={onCreateOption}
      />
    </div>
  );
}
