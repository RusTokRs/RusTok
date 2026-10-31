import { registerAdminModule } from '@/modules/registry';
import { mediaNavItems } from './nav';

registerAdminModule({
  id: 'media',
  name: 'Media',
  navItems: mediaNavItems
});

export { mediaNavItems } from './nav';
export { default as MediaPage, default as MediaAdminPage } from './pages/media-page';
export { MediaActions } from './components/media-actions';
export * from './api/media';
