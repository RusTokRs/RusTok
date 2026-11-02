/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import { canonicalizeLocale } from '@rustok/next-fluent/utils';
import type { ProductWriteScope } from './write-attempt';

export type AttributeDefinitionCopy = {
  label: string;
  helpText: string | null;
  facetLabel: string | null;
  seoLabel: string | null;
};
export type AttributeDefinitionPatch = {
  translation?: AttributeDefinitionCopy | null;
  isFilterable?: boolean | null;
  isSearchable?: boolean | null;
  isSortable?: boolean | null;
  isComparable?: boolean | null;
  showOnStorefront?: boolean | null;
  showInAdminGrid?: boolean | null;
  searchWeight?: number | null;
  position?: number | null;
};
export type AttributeOptionDefinitionPatch = {
  label?: string | null;
  position?: number | null;
};
export type AttributeDefinitionAction = 'update' | 'archive' | 'restore';
export type UpdateAttributeDefinitionInput = {
  attributeId: string;
  optionId: string | null;
  locale: string;
  expectedState: string;
  action: AttributeDefinitionAction;
  attributePatch: AttributeDefinitionPatch | null;
  optionPatch: AttributeOptionDefinitionPatch | null;
};
export type AttributeDefinitionWriteRequest = {
  scope: ProductWriteScope;
  id: string;
  input: UpdateAttributeDefinitionInput;
};
export type AttributeLifecycleRequest = {
  attributeId: string | null;
  optionId: string | null;
  includeArchived: boolean;
};
export type AttributeDefinition = {
  id: string;
  code: string;
  valueType: string;
  scope: string;
  isLocalized: boolean;
  isFilterable: boolean;
  isSearchable: boolean;
  isSortable: boolean;
  isComparable: boolean;
  showOnStorefront: boolean;
  showInAdminGrid: boolean;
  searchWeight: number;
  position: number;
  isArchived: boolean;
  translation: AttributeDefinitionCopy | null;
  stateToken: string;
};
export type AttributeOptionDefinition = {
  id: string;
  attributeId: string;
  code: string;
  position: number;
  isArchived: boolean;
  attributeArchived: boolean;
  label: string | null;
  stateToken: string;
};
export const ATTRIBUTE_REFERENCE_KINDS = [
  'product_value',
  'variant_value',
  'schema_binding',
  'category_binding',
  'clone_snapshot',
  'variant_axis',
  'allowed_option',
  'attribute_group',
  'channel_setting',
  'option_rule',
  'virtual_category'
] as const;
export type AttributeReferenceKind = (typeof ATTRIBUTE_REFERENCE_KINDS)[number];
export type AttributeReferenceImpact = {
  references: { kind: AttributeReferenceKind; resourceId: string }[];
  hasMore: boolean;
  inspectionComplete: boolean;
};
export type AttributeDefinitionRefusal =
  'stale' | 'in_use' | 'inspection_limit' | 'not_found' | 'parent_archived';
export type AttributeDefinitionResult = {
  attributeId: string;
  optionId: string | null;
  applied: boolean;
  stateToken: string | null;
  refusal: AttributeDefinitionRefusal | null;
  impact: AttributeReferenceImpact | null;
};
export type AttributeLifecycleWorkspace = {
  attributes: AttributeDefinition[];
  options: AttributeOptionDefinition[] | null;
  impact: AttributeReferenceImpact | null;
};

const object = (v: unknown): v is Record<string, unknown> =>
  !!v && typeof v === 'object' && !Array.isArray(v);
const only = (v: Record<string, unknown>, keys: readonly string[]) =>
  Object.keys(v).every((key) => keys.includes(key));
export const isDefinitionId = (v: unknown): v is string =>
  typeof v === 'string' &&
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(v) &&
  !/^0{8}-0{4}-0{4}-0{4}-0{12}$/.test(v);
export function isDefinitionLocale(v: unknown): v is string {
  if (
    typeof v !== 'string' ||
    v.length > 32 ||
    v === 'und' ||
    canonicalizeLocale(v) !== v
  )
    return false;
  try {
    return new Intl.Locale(v).baseName === v;
  } catch {
    return false;
  }
}
const token = (v: unknown): v is string =>
  typeof v === 'string' && /^[a-f0-9]{64}$/.test(v);
const int32 = (v: unknown): v is number =>
  Number.isInteger(v) &&
  (v as number) >= -2147483648 &&
  (v as number) <= 2147483647;
const nonnegative = (v: unknown): v is number =>
  Number.isInteger(v) && (v as number) >= 0 && (v as number) <= 2147483647;
// Rust String/serde cannot represent lone UTF-16 surrogates. Reject them before
// persistence, and use Unicode White_Space (Rust trim), not JS trim's BOM/NEL set.
const unicodeText = (v: unknown): v is string =>
  typeof v === 'string' && !/[\uD800-\uDFFF]/u.test(v);
const text = (v: unknown, max: number) =>
  unicodeText(v) &&
  /\P{White_Space}/u.test(v) &&
  [...v].length <= max &&
  !v.includes('\0');
const nullableText = (v: unknown) => v === null || unicodeText(v);
const utf8Length = (v: string) => new TextEncoder().encode(v).length;
const flags = [
  'isFilterable',
  'isSearchable',
  'isSortable',
  'isComparable',
  'showOnStorefront',
  'showInAdminGrid'
] as const;
const isCopy = (v: unknown): v is AttributeDefinitionCopy =>
  object(v) &&
  only(v, ['label', 'helpText', 'facetLabel', 'seoLabel']) &&
  text(v.label, 255) &&
  (v.helpText === null ||
    (unicodeText(v.helpText) &&
      utf8Length(v.helpText) <= 4096 &&
      !v.helpText.includes('\0'))) &&
  [v.facetLabel, v.seoLabel].every(
    (value) => value === null || text(value, 255)
  );
const isStoredCopy = (v: unknown): v is AttributeDefinitionCopy =>
  object(v) &&
  only(v, ['label', 'helpText', 'facetLabel', 'seoLabel']) &&
  unicodeText(v.label) &&
  [v.helpText, v.facetLabel, v.seoLabel].every(nullableText);
function isAttributePatch(v: unknown): v is AttributeDefinitionPatch {
  return (
    object(v) &&
    only(v, ['translation', ...flags, 'searchWeight', 'position']) &&
    Object.values(v).some((value) => value !== null && value !== undefined) &&
    (v.translation === undefined ||
      v.translation === null ||
      isCopy(v.translation)) &&
    flags.every(
      (key) =>
        v[key] === undefined || v[key] === null || typeof v[key] === 'boolean'
    ) &&
    ['position', 'searchWeight'].every(
      (key) => v[key] === undefined || v[key] === null || nonnegative(v[key])
    )
  );
}
function isOptionPatch(v: unknown): v is AttributeOptionDefinitionPatch {
  return (
    object(v) &&
    only(v, ['label', 'position']) &&
    Object.values(v).some((value) => value !== null && value !== undefined) &&
    (v.label === undefined || v.label === null || text(v.label, 255)) &&
    (v.position === undefined || v.position === null || nonnegative(v.position))
  );
}
export function isAttributeDefinitionInput(
  v: unknown
): v is UpdateAttributeDefinitionInput {
  if (
    !object(v) ||
    !only(v, [
      'attributeId',
      'optionId',
      'locale',
      'expectedState',
      'action',
      'attributePatch',
      'optionPatch'
    ]) ||
    !isDefinitionId(v.attributeId) ||
    !(v.optionId === null || isDefinitionId(v.optionId)) ||
    !token(v.expectedState) ||
    !isDefinitionLocale(v.locale)
  )
    return false;
  if (v.action === 'archive' || v.action === 'restore')
    return v.attributePatch === null && v.optionPatch === null;
  return (
    v.action === 'update' &&
    (v.optionId === null
      ? isAttributePatch(v.attributePatch) && v.optionPatch === null
      : v.attributePatch === null && isOptionPatch(v.optionPatch))
  );
}
export function isAttributeLifecycleRequest(
  v: unknown
): v is AttributeLifecycleRequest {
  return (
    object(v) &&
    only(v, ['attributeId', 'optionId', 'includeArchived']) &&
    (v.attributeId === null || isDefinitionId(v.attributeId)) &&
    (v.optionId === null ||
      (v.attributeId !== null && isDefinitionId(v.optionId))) &&
    typeof v.includeArchived === 'boolean'
  );
}
export function isAttributeDefinitionRequest(
  v: unknown
): v is AttributeDefinitionWriteRequest {
  return (
    object(v) &&
    only(v, ['scope', 'id', 'input']) &&
    object(v.scope) &&
    only(v.scope, ['tenantId', 'actorId']) &&
    isDefinitionId(v.scope.tenantId) &&
    isDefinitionId(v.scope.actorId) &&
    isDefinitionId(v.id) &&
    isAttributeDefinitionInput(v.input) &&
    v.input.attributeId === v.id
  );
}
export function isAttributeImpact(v: unknown): v is AttributeReferenceImpact {
  return (
    object(v) &&
    only(v, ['references', 'hasMore', 'inspectionComplete']) &&
    typeof v.hasMore === 'boolean' &&
    typeof v.inspectionComplete === 'boolean' &&
    Array.isArray(v.references) &&
    v.references.length <= 8 &&
    v.references.every(
      (ref) =>
        object(ref) &&
        only(ref, ['kind', 'resourceId']) &&
        isDefinitionId(ref.resourceId) &&
        ATTRIBUTE_REFERENCE_KINDS.includes(ref.kind as AttributeReferenceKind)
    )
  );
}
export function isAttributeDefinitionResult(
  v: unknown,
  input: UpdateAttributeDefinitionInput
): v is AttributeDefinitionResult {
  if (
    !object(v) ||
    !only(v, [
      'attributeId',
      'optionId',
      'applied',
      'stateToken',
      'refusal',
      'impact'
    ]) ||
    v.attributeId !== input.attributeId ||
    v.optionId !== input.optionId
  )
    return false;
  if (v.applied === true)
    return token(v.stateToken) && v.refusal === null && v.impact === null;
  if (v.applied !== false) return false;
  switch (v.refusal) {
    case 'not_found':
      return v.stateToken === null && v.impact === null;
    case 'stale':
      return token(v.stateToken) && v.impact === null;
    case 'parent_archived':
      return (
        input.action === 'restore' &&
        input.optionId !== null &&
        token(v.stateToken) &&
        v.impact === null
      );
    case 'in_use':
      return (
        input.action === 'archive' &&
        token(v.stateToken) &&
        isAttributeImpact(v.impact) &&
        v.impact.inspectionComplete &&
        (v.impact.references.length > 0 || v.impact.hasMore)
      );
    case 'inspection_limit':
      return (
        input.action === 'archive' &&
        token(v.stateToken) &&
        isAttributeImpact(v.impact) &&
        !v.impact.inspectionComplete
      );
    default:
      return false;
  }
}
export function isAttributeWorkspace(
  v: unknown
): v is AttributeLifecycleWorkspace {
  if (
    !object(v) ||
    !only(v, ['attributes', 'options', 'impact']) ||
    !Array.isArray(v.attributes) ||
    v.attributes.length > 1000 ||
    !(v.impact === null || isAttributeImpact(v.impact))
  )
    return false;
  if (
    !v.attributes.every(
      (row) =>
        object(row) &&
        only(row, [
          'id',
          'code',
          'valueType',
          'scope',
          'isLocalized',
          ...flags,
          'searchWeight',
          'position',
          'isArchived',
          'translation',
          'stateToken'
        ]) &&
        isDefinitionId(row.id) &&
        typeof row.code === 'string' &&
        typeof row.valueType === 'string' &&
        ['product', 'variant', 'both'].includes(row.scope as string) &&
        ['isLocalized', ...flags, 'isArchived'].every(
          (key) => typeof row[key] === 'boolean'
        ) &&
        int32(row.position) &&
        int32(row.searchWeight) &&
        (row.translation === null || isStoredCopy(row.translation)) &&
        token(row.stateToken)
    )
  )
    return false;
  if (new Set(v.attributes.map((row) => row.id)).size !== v.attributes.length)
    return false;
  return (
    v.options === null ||
    (Array.isArray(v.options) &&
      v.options.length <= 1000 &&
      new Set(v.options.map((row: unknown) => (object(row) ? row.id : null)))
        .size === v.options.length &&
      v.options.every(
        (row) =>
          object(row) &&
          only(row, [
            'id',
            'attributeId',
            'code',
            'position',
            'isArchived',
            'attributeArchived',
            'label',
            'stateToken'
          ]) &&
          isDefinitionId(row.id) &&
          isDefinitionId(row.attributeId) &&
          typeof row.code === 'string' &&
          int32(row.position) &&
          typeof row.isArchived === 'boolean' &&
          typeof row.attributeArchived === 'boolean' &&
          nullableText(row.label) &&
          token(row.stateToken)
      ))
  );
}

export function attributePresentationJson(
  workspace: AttributeLifecycleWorkspace,
  optionId: string | null
): string | null {
  const attribute = workspace.attributes[0];
  if (!attribute) return null;
  if (optionId !== null) {
    const option = workspace.options?.find((option) => option.id === optionId);
    return option
      ? JSON.stringify(
          { label: option.label, position: option.position },
          null,
          2
        )
      : null;
  }
  return JSON.stringify(
    {
      translation: attribute.translation,
      ...Object.fromEntries(flags.map((key) => [key, attribute[key]])),
      searchWeight: attribute.searchWeight,
      position: attribute.position
    },
    null,
    2
  );
}
export function prepareAttributeDefinition(
  scope: ProductWriteScope,
  workspace: AttributeLifecycleWorkspace,
  optionId: string | null,
  action: AttributeDefinitionAction,
  locale: string,
  patchJson: string
): AttributeDefinitionWriteRequest {
  const attribute = workspace.attributes[0];
  if (
    !isAttributeWorkspace(workspace) ||
    !attribute ||
    workspace.attributes.length !== 1 ||
    workspace.options === null ||
    workspace.options.some(
      (row) =>
        row.attributeId !== attribute.id ||
        row.attributeArchived !== attribute.isArchived
    ) ||
    (action === 'update' && utf8Length(patchJson) > 8192)
  )
    throw new Error('Invalid definition selection');
  const selected =
    optionId === null
      ? attribute
      : workspace.options?.find((option) => option.id === optionId);
  if (!selected) throw new Error('Definition is unavailable');
  const patch = action === 'update' ? JSON.parse(patchJson) : null;
  // A supplied translation replaces one complete exact-locale copy. Materialize optional
  // nullable members once BEFORE journaling, just like the Rust DTO deserializer.
  if (optionId === null && object(patch) && object(patch.translation)) {
    for (const key of ['helpText', 'facetLabel', 'seoLabel']) {
      if (patch.translation[key] === undefined) patch.translation[key] = null;
    }
  }
  const request = {
    scope,
    id: attribute.id,
    input: {
      attributeId: attribute.id,
      optionId,
      locale,
      expectedState: selected.stateToken,
      action,
      attributePatch: action === 'update' && optionId === null ? patch : null,
      optionPatch: action === 'update' && optionId !== null ? patch : null
    }
  };
  if (!isAttributeDefinitionRequest(request))
    throw new Error('Invalid definition request');
  return structuredClone(request);
}
export const attributeDefinitionRefusalCode = (
  refusal: AttributeDefinitionRefusal
) => `PRODUCT_ATTRIBUTE_DEFINITION_${refusal.toUpperCase()}`;
export function isAttributeDefinitionRefusalCode(
  code: unknown
): code is string {
  return (
    typeof code === 'string' &&
    [
      'PRODUCT_VALIDATION',
      ...[
        'stale',
        'in_use',
        'inspection_limit',
        'not_found',
        'parent_archived'
      ].map((refusal) =>
        attributeDefinitionRefusalCode(refusal as AttributeDefinitionRefusal)
      )
    ].includes(code)
  );
}

export function attributeOutcomeKey(
  outcome: import('./write-attempt').ProductWriteOutcome
) {
  if (outcome.status === 'confirmed')
    return 'attributeLifecycle-confirmed' as const;
  if (outcome.status === 'unknown')
    return 'attributeLifecycle-unknown' as const;
  switch (outcome.definition?.refusal) {
    case 'stale':
      return 'attributeLifecycle-stale' as const;
    case 'in_use':
      return 'attributeLifecycle-inUse' as const;
    case 'inspection_limit':
      return 'attributeLifecycle-limit' as const;
    case 'not_found':
      return 'attributeLifecycle-notFound' as const;
    case 'parent_archived':
      return 'attributeLifecycle-parentArchived' as const;
    default:
      return 'attributeLifecycle-invalid' as const;
  }
}
export function attributeReferenceKey(kind: AttributeReferenceKind) {
  const keys = {
    product_value: 'attributeLifecycle-refProductValue',
    variant_value: 'attributeLifecycle-refVariantValue',
    schema_binding: 'attributeLifecycle-refSchemaBinding',
    category_binding: 'attributeLifecycle-refCategoryBinding',
    clone_snapshot: 'attributeLifecycle-refCloneSnapshot',
    variant_axis: 'attributeLifecycle-refVariantAxis',
    allowed_option: 'attributeLifecycle-refAllowedOption',
    attribute_group: 'attributeLifecycle-refAttributeGroup',
    channel_setting: 'attributeLifecycle-refChannelSetting',
    option_rule: 'attributeLifecycle-refOptionRule',
    virtual_category: 'attributeLifecycle-refVirtualCategory'
  } as const;
  return keys[kind];
}
