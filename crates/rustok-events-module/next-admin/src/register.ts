import { registerAdminModule } from '@/modules/registry';

import { eventsNavItems } from './nav';

registerAdminModule({
  id: 'events',
  name: 'Events & Outbox',
  navItems: eventsNavItems
});
