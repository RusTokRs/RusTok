import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { AttributesPage } from '@rustok/product-admin';
import {
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

export default async function Page() {
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
