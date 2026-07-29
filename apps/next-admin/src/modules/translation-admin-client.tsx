'use client';

import {
  TranslationAdminPage,
  type TranslationAdminPageProps
} from '@rustok/translation-admin';
import { graphqlRequest } from '@/shared/api/graphql';

type TranslationAdminClientProps = Omit<TranslationAdminPageProps, 'graphql'>;

export function TranslationAdminClient(props: TranslationAdminClientProps) {
  return <TranslationAdminPage {...props} graphql={graphqlRequest} />;
}
