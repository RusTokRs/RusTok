import { registerAdminModule } from '@/modules/registry';
import { eventsNavItems } from './nav';

registerAdminModule({
  id: 'events',
  name: 'Events & Outbox',
  navItems: eventsNavItems
});

export { eventsNavItems } from './nav';
export { EventsPage } from './pages/events-page';
export * from './api/events';
