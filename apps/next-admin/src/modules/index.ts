// Feature modules — each registers its nav items via registerAdminModule()
// To add a new module: create src/features/<name>/index.ts and add import here
import '@/features/blog';
import '@/features/workflow';
// Core module UIs
import '@/features/rbac';
import '@/features/email';
import '@/features/cache';

export type { AdminModule } from './types';
export { registerAdminModule, getAdminModules, getAdminNavItems } from './registry';
