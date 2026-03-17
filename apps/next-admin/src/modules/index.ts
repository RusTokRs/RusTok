// Module UI packages — import side-effect modules here so they self-register
import '@rustok/blog-admin';
import '@rustok/workflow-admin';

export type { AdminModule } from './types';
export { registerAdminModule, getAdminModules, getAdminNavItems } from './registry';
