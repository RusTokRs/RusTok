import { registerAdminModule } from "@/modules/registry";
import { iggyConnectorNavItems } from "./nav";

registerAdminModule({
  id: "iggy_connector",
  name: "Iggy Connector",
  navItems: iggyConnectorNavItems,
});

export { iggyConnectorNavItems } from "./nav";
export { IggyConnectorPage } from "./pages/iggy-connector-page";
export * from "./api/iggy-connector";
