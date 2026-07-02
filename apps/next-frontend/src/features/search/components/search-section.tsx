"use client";

import React from "react";

import SearchStorefrontPage, {
  type SearchCatalogFilterOption,
} from "../../../../packages/search/src";
import { fetchCatalogSearchOptions } from "../../../../packages/rustok-product/src";

export type SearchSectionProps = {
  locale: string;
  enabledModules: string[];
  tenantSlug: string | null;
};

export function SearchSection({
  locale,
  enabledModules,
  tenantSlug,
}: SearchSectionProps): React.JSX.Element {
  const productEnabled = enabledModules.includes("product");
  const [categoryOptions, setCategoryOptions] = React.useState<
    SearchCatalogFilterOption[]
  >([]);
  const [attributeOptions, setAttributeOptions] = React.useState<
    SearchCatalogFilterOption[]
  >([]);

  React.useEffect(() => {
    let cancelled = false;
    if (!productEnabled || !locale.trim()) {
      setCategoryOptions([]);
      setAttributeOptions([]);
      return;
    }

    void fetchCatalogSearchOptions({ locale, tenantSlug })
      .then((options) => {
        if (cancelled) {
          return;
        }
        setCategoryOptions(options.categoryOptions);
        setAttributeOptions(options.attributeOptions);
      })
      .catch(() => {
        if (cancelled) {
          return;
        }
        setCategoryOptions([]);
        setAttributeOptions([]);
      });

    return () => {
      cancelled = true;
    };
  }, [locale, productEnabled, tenantSlug]);

  return (
    <SearchStorefrontPage
      locale={locale}
      tenantSlug={tenantSlug}
      categoryOptions={categoryOptions}
      attributeOptions={attributeOptions}
    />
  );
}
