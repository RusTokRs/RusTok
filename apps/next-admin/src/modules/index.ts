// Admin modules register their nav through module-owned package entrypoints.
// Host shell code should not import business UI feature folders directly.
import '../../packages/blog/src';
import '../../packages/rustok-product/src';
import '../../packages/workflow/src';
import '../../packages/rbac/src';
import '../../packages/email/src';
import '../../packages/cache/src';
import '../../packages/events/src';

export type { AdminModule } from './types';
export {
  registerAdminModule,
  getAdminModules,
  getAdminNavItems
} from './registry';
