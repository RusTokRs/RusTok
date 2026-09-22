import { delay } from '@/shared/lib/timing';
import { AreaGraph } from '@/features/overview/components/area-graph';

export default async function AreaStats() {
  await delay(2000);
  return <AreaGraph />;
}
