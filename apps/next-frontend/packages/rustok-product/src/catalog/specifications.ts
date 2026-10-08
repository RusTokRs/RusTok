/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import type {
  StorefrontProductAttribute,
  StorefrontProductDetail,
} from "../api/types";

/**
 * Specifications of the product detail page.
 *
 * The Product owner ships display-ready attributes: labels and option labels resolved for the
 * requested locale, values formatted, service and non-storefront attributes filtered out before
 * they leave the owner. The one vocabulary the owner cannot phrase is the boolean one — `true` is
 * a stored value, not a string with locale copy — so the storefronts own that wording and this
 * module maps it. Nothing else is re-formatted here: re-deriving amounts or option labels in a
 * storefront would duplicate the owner's localization.
 */
export type ProductSpecificationLabels = {
  title: string;
  yes: string;
  no: string;
};

/** One row of the specifications table. */
export type ProductSpecificationRow = {
  code: string;
  label: string;
  value: string;
};

/** Copy of the specifications block for the requested locale. */
export function buildProductSpecificationLabels(
  locale?: string | null,
): ProductSpecificationLabels {
  const isRu = (locale ?? "").trim().toLowerCase().startsWith("ru");
  return isRu
    ? { title: "Характеристики", yes: "Да", no: "Нет" }
    : { title: "Specifications", yes: "Yes", no: "No" };
}

/**
 * Localizes one owner value.
 *
 * Booleans become the locale's yes/no pair; every other value is printed as the owner sent it,
 * including an unexpected boolean payload, which is left untouched rather than dropped.
 */
export function formatSpecificationValue(
  attribute: Pick<StorefrontProductAttribute, "valueType">,
  value: string,
  labels: ProductSpecificationLabels,
): string {
  if ((attribute.valueType ?? "").trim().toLowerCase() !== "boolean") {
    return value;
  }
  switch (value.trim().toLowerCase()) {
    case "true":
      return labels.yes;
    case "false":
      return labels.no;
    default:
      return value;
  }
}

/**
 * Rows of the specifications table, in the order the owner resolved.
 *
 * Attributes without values are dropped: an empty row would claim the product has a
 * characteristic it has no value for. The label falls back to the attribute code so a missing
 * translation never renders as an empty cell.
 */
export function buildProductSpecifications(
  product: Pick<StorefrontProductDetail, "attributes">,
  labels: ProductSpecificationLabels,
): ProductSpecificationRow[] {
  const rows: ProductSpecificationRow[] = [];
  for (const attribute of product.attributes ?? []) {
    const values = (attribute.values ?? [])
      .map((value) => formatSpecificationValue(attribute, value.text, labels))
      .filter((value) => value.trim().length > 0);
    if (values.length === 0) continue;
    rows.push({
      code: attribute.code,
      label: attribute.label.trim() || attribute.code,
      value: values.join(" · "),
    });
  }
  return rows;
}
