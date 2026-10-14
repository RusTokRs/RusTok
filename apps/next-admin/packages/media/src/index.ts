import { registerAdminModule } from '@/modules/registry';
import { mediaNavItems } from './nav';

registerAdminModule({
  id: 'media',
  name: 'Media',
  navItems: mediaNavItems
});

export { mediaNavItems } from './nav';
export { default as MediaPage } from './pages/media-page';
export * from './api/media';
