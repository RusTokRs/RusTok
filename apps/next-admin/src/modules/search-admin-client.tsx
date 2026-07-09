'use client';

import {
  SearchAdminPage,
  type SearchAdminPageProps
} from '../../packages/search/src';
import { graphqlRequest } from '@/shared/api/graphql';

type SearchAdminClientProps = Omit<SearchAdminPageProps, 'graphql'>;

export function SearchAdminClient(props: SearchAdminClientProps) {
  return <SearchAdminPage {...props} graphql={graphqlRequest} />;
}
