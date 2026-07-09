import { registerStorefrontModule } from "@/modules/registry";

import { SearchSection } from "./components/search-section";

registerStorefrontModule({
  id: "search-catalog",
  moduleSlug: "search",
  slot: "home:afterHero",
  order: 30,
  render: ({ locale, enabledModules, tenantSlug, graphql }) => (
    <SearchSection
      locale={locale}
      enabledModules={enabledModules}
      tenantSlug={tenantSlug}
      graphql={graphql}
    />
  ),
});

export { SearchSection } from "./components/search-section";
