import { Card, CardContent, CardHeader } from '@/shared/ui/shadcn/card';
import { Skeleton } from '@/shared/ui/shadcn/skeleton';

const BAR_HEIGHT_CLASSES = [
  'h-[28%]',
  'h-[46%]',
  'h-[64%]',
  'h-[38%]',
  'h-[78%]',
  'h-[56%]',
  'h-[88%]',
  'h-[42%]',
  'h-[70%]',
  'h-[34%]',
  'h-[82%]',
  'h-[52%]'
] as const;

export function BarGraphSkeleton() {
  return (
    <Card>
      <CardHeader className='flex flex-col items-stretch space-y-0 border-b p-0 sm:flex-row'>
        <div className='flex flex-1 flex-col justify-center gap-1 px-6 py-5 sm:py-6'>
          <Skeleton className='h-6 w-[180px]' />
          <Skeleton className='h-4 w-[250px]' />
        </div>
        <div className='flex'>
          {[1, 2].map((i) => (
            <div
              key={i}
              className='relative flex flex-1 flex-col justify-center gap-1 border-t px-6 py-4 text-left even:border-l sm:border-t-0 sm:border-l sm:px-8 sm:py-6'
            >
              <Skeleton className='h-3 w-[80px]' />
              <Skeleton className='h-8 w-[100px] sm:h-10' />
            </div>
          ))}
        </div>
      </CardHeader>
      <CardContent className='px-2 sm:p-6'>
        <div className='flex aspect-auto h-[280px] w-full items-end justify-around gap-2 pt-8'>
          {BAR_HEIGHT_CLASSES.map((heightClass, index) => (
            <Skeleton key={index} className={`w-full ${heightClass}`} />
          ))}
        </div>
      </CardContent>
    </Card>
  );
}
