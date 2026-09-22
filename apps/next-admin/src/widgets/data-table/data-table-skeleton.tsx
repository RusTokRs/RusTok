import { Skeleton } from '@/shared/ui/shadcn/skeleton';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from '@/widgets/data-table';
import { cn } from '@/shared/lib/utils';

interface DataTableSkeletonProps extends React.ComponentProps<'div'> {
  columnCount: number;
  rowCount?: number;
  filterCount?: number;
  withViewOptions?: boolean;
  withPagination?: boolean;
}

export function DataTableSkeleton({
  columnCount,
  rowCount = 10,
  filterCount = 0,
  withViewOptions = true,
  withPagination = true,
  className,
  ...props
}: DataTableSkeletonProps) {
  return (
    <div className={cn('flex flex-1 flex-col space-y-4', className)} {...props}>
      <div className='flex w-full items-center justify-between gap-2 overflow-auto p-1'>
        <div className='flex flex-1 items-center gap-2'>
          {filterCount > 0
            ? Array.from({ length: filterCount }).map((_, i) => (
                <Skeleton key={i} className='h-7 w-[4.5rem] border-dashed' />
              ))
            : null}
        </div>
        {withViewOptions ? (
          <Skeleton className='ml-auto hidden h-7 w-[4.5rem] lg:flex' />
        ) : null}
      </div>

      <div className='flex-1 rounded-md border'>
        <Table>
          <TableHeader>
            {Array.from({ length: 1 }).map((_, rowIndex) => (
              <TableRow key={rowIndex} className='hover:bg-transparent'>
                {Array.from({ length: columnCount }).map((_, columnIndex) => (
                  <TableHead key={columnIndex}>
                    <Skeleton className='h-6 w-full' />
                  </TableHead>
                ))}
              </TableRow>
            ))}
          </TableHeader>
          <TableBody>
            {Array.from({ length: rowCount }).map((_, rowIndex) => (
              <TableRow key={rowIndex} className='hover:bg-transparent'>
                {Array.from({ length: columnCount }).map((_, columnIndex) => (
                  <TableCell key={columnIndex}>
                    <Skeleton className='h-6 w-full' />
                  </TableCell>
                ))}
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
      {withPagination ? (
        <div className='flex w-full items-center justify-between gap-4 overflow-auto p-1 sm:gap-8'>
          <Skeleton className='h-7 w-40 shrink-0' />
          <div className='flex items-center gap-4 sm:gap-6 lg:gap-8'>
            <div className='flex items-center gap-2'>
              <Skeleton className='h-7 w-24' />
              <Skeleton className='h-7 w-[4.5rem]' />
            </div>
            <div className='flex items-center justify-center text-sm font-medium'>
              <Skeleton className='h-7 w-20' />
            </div>
            <div className='flex items-center gap-2'>
              <Skeleton className='hidden size-7 lg:block' />
              <Skeleton className='size-7' />
              <Skeleton className='size-7' />
              <Skeleton className='hidden size-7 lg:block' />
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
