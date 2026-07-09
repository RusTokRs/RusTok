'use client';

import {
  CartPromotionsTemplate,
  OrderChangesTemplate,
  ReturnDecisionsTemplate,
  ShippingProfilesTemplate,
  type GqlOpts
} from '@rustok/commerce-admin';
import { graphqlRequest } from '@/shared/api/graphql';

type HostGqlOpts = Omit<GqlOpts, 'graphql'>;

function withGraphql(opts: HostGqlOpts): GqlOpts {
  return { ...opts, graphql: graphqlRequest };
}

export function ShippingProfilesClient({ opts }: { opts: HostGqlOpts }) {
  return <ShippingProfilesTemplate opts={withGraphql(opts)} />;
}

export function CartPromotionsClient({ opts }: { opts: HostGqlOpts }) {
  return <CartPromotionsTemplate opts={withGraphql(opts)} />;
}

export function OrderChangesClient({ opts }: { opts: HostGqlOpts }) {
  return <OrderChangesTemplate opts={withGraphql(opts)} />;
}

export function ReturnDecisionsClient({ opts }: { opts: HostGqlOpts }) {
  return <ReturnDecisionsTemplate opts={withGraphql(opts)} />;
}
