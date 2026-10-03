import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { AttributesPage } from '@rustok/product-admin';
import {
  createAttributeAction,
  createAttributeOptionAction,
  createAttributeSchemaAction
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
      />
    </PageContainer>
  );
}
