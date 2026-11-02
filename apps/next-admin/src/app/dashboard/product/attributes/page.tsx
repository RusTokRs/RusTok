import { getLocale } from '@rustok/next-fluent/server';
import { auth } from '@/auth';
import { fetchEffectiveFormAction } from '../actions';
import { PageContainer } from '@/widgets/app-shell';
import {
  AttributesPage,
  ATTRIBUTE_DEFINITION_INTENT_QUERY_KEY
} from '@rustok/product-admin';
import {
  readAttributeLifecycleAction,
  updateAttributeDefinitionAction,
  bindCategoryAttributeAction,
  bindSchemaAttributeAction,
  createAttributeAction,
  createAttributeOptionAction,
  createAttributeSchemaAction,
  createCategoryAttributeGroupAction,
  createSchemaAttributeGroupAction,
  setCategorySchemaModeAction
} from './actions';

export const metadata = {
  title: 'RusTok Admin: Attributes'
};

export default async function Page({
  searchParams
}: {
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const query = await searchParams;
  const selected = query.attribute_id;
  const attributeId = typeof selected === 'string' ? selected : null;
  const selectedOption = query.option_id;
  const optionId = typeof selectedOption === 'string' ? selectedOption : null;
  const invalidSelection =
    Array.isArray(selected) ||
    Array.isArray(selectedOption) ||
    (attributeId === null &&
      (selectedOption !== undefined ||
        query[ATTRIBUTE_DEFINITION_INTENT_QUERY_KEY] !== undefined));
  const locale = await getLocale();
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;

  return (
    <PageContainer
      pageTitle='Product Attributes'
      pageDescription='Configure custom attributes, option lists, and category schemas.'
    >
      <AttributesPage
        locale={locale}
        definition={{
          scope: { tenantId: tenantId ?? '', actorId: session?.user?.id ?? '' },
          attributeId,
          optionId,
          invalidSelection,
          read: readAttributeLifecycleAction,
          write: updateAttributeDefinitionAction
        }}
        fetchEffectiveForm={fetchEffectiveFormAction}
        token={token}
        tenantSlug={tenantSlug}
        tenantId={tenantId}
        onCreateAttribute={createAttributeAction}
        onCreateOption={createAttributeOptionAction}
        onCreateSchema={createAttributeSchemaAction}
        onSetSchemaMode={setCategorySchemaModeAction}
        onCreateSchemaGroup={createSchemaAttributeGroupAction}
        onCreateCategoryGroup={createCategoryAttributeGroupAction}
        onBindSchemaAttribute={bindSchemaAttributeAction}
        onBindCategoryAttribute={bindCategoryAttributeAction}
      />
    </PageContainer>
  );
}
