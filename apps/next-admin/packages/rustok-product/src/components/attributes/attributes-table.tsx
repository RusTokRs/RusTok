'use client';

import * as React from 'react';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from '@/widgets/data-table/table';
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

      <div className='rounded-md border'>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className='w-[240px]'>Label</TableHead>
              <TableHead>Code</TableHead>
              <TableHead>Value Type</TableHead>
              <TableHead className='text-center'>Localized</TableHead>
              <TableHead className='text-center'>Filterable</TableHead>
              <TableHead className='text-center'>Sortable</TableHead>
              <TableHead className='text-center'>Storefront</TableHead>
              <TableHead className='text-right'>Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {filteredAttributes.length === 0 ? (
              <TableRow>
                <TableCell
                  colSpan={8}
                  className='text-muted-foreground py-8 text-center text-sm'
                >
                  <div className='flex flex-col items-center justify-center gap-1.5'>
                    <Sliders className='text-muted-foreground/50 h-8 w-8' />
                    <p>No product attributes found.</p>
                  </div>
                </TableCell>
              </TableRow>
            ) : (
              filteredAttributes.map((attr) => {
                const isOptionType =
                  attr.valueType === 'option' ||
                  attr.valueType === 'multi_option';

                return (
                  <TableRow key={attr.id}>
                    <TableCell className='text-sm font-medium'>
                      {attr.label}
                    </TableCell>
                    <TableCell>
                      <code className='bg-muted rounded px-1.5 py-0.5 font-mono text-xs'>
                        {attr.code}
                      </code>
                    </TableCell>
                    <TableCell>
                      <Badge
                        variant='outline'
                        className='text-xs font-normal capitalize'
                      >
                        {attr.valueType.replace('_', ' ')}
                      </Badge>
                    </TableCell>
                    <TableCell className='text-center'>
                      {attr.isLocalized ? (
                        <Check className='inline h-4 w-4 text-emerald-600' />
                      ) : (
                        <Minus className='text-muted-foreground/40 inline h-3.5 w-3.5' />
                      )}
                    </TableCell>
                    <TableCell className='text-center'>
                      {attr.isFilterable ? (
                        <Check className='inline h-4 w-4 text-emerald-600' />
                      ) : (
                        <Minus className='text-muted-foreground/40 inline h-3.5 w-3.5' />
                      )}
                    </TableCell>
                    <TableCell className='text-center'>
                      {attr.isSortable ? (
                        <Check className='inline h-4 w-4 text-emerald-600' />
                      ) : (
                        <Minus className='text-muted-foreground/40 inline h-3.5 w-3.5' />
                      )}
                    </TableCell>
                    <TableCell className='text-center'>
                      {attr.showOnStorefront ? (
                        <Check className='inline h-4 w-4 text-emerald-600' />
                      ) : (
                        <Minus className='text-muted-foreground/40 inline h-3.5 w-3.5' />
                      )}
                    </TableCell>
                    <TableCell className='text-right'>
                      {isOptionType ? (
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
                      )}
                    </TableCell>
                  </TableRow>
                );
              })
            )}
          </TableBody>
        </Table>
      </div>

      <AttributeOptionsDialog
        attribute={selectedAttribute}
        open={optionDialogOpen}
        onOpenChange={setOptionDialogOpen}
        onCreateOption={onCreateOption}
      />
    </div>
  );
}
