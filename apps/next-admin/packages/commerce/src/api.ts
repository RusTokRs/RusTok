import type {
  ShippingProfile,
  ShippingProfileList,
  CreateShippingProfileInput,
  UpdateShippingProfileInput,
  CartPromotionPreview,
  CartSnapshot,
  AdminCartPromotionInput,
  OrderChange,
  OrderChangeList,
  ExchangeDifferenceRefundInput,
  CreateReturnDecisionInput,
  ReturnDecisionResponse
} from './types';

export type GqlOpts = {
  graphql: AdminGraphqlExecutor;
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
};

export type AdminGraphqlExecutor = <V, T>(
  query: string,
  variables?: V,
  token?: string | null,
  tenantSlug?: string | null,
  options?: { graphqlUrl?: string; tenantId?: string | null }
) => Promise<T>;

// GRAPHQL QUERIES & MUTATIONS

const SHIPPING_PROFILES_QUERY = `
query CommerceAdminShippingProfiles($tenantId: UUID!, $filter: ShippingProfilesFilter) {
  shippingProfiles(tenantId: $tenantId, filter: $filter) {
    total
    page
    perPage
    hasNext
    items {
      id
      slug
      active
      metadata
      createdAt
      updatedAt
      translations {
        locale
        name
        description
      }
    }
  }
}`;

const SHIPPING_PROFILE_QUERY = `
query CommerceAdminShippingProfile($tenantId: UUID!, $id: UUID!) {
  shippingProfile(tenantId: $tenantId, id: $id) {
    id
    slug
    active
    metadata
    createdAt
    updatedAt
    translations {
      locale
      name
      description
    }
  }
}`;

const CREATE_SHIPPING_PROFILE_MUTATION = `
mutation CommerceAdminCreateShippingProfile($tenantId: UUID!, $input: CreateShippingProfileInputObject!) {
  createShippingProfile(tenantId: $tenantId, input: $input) {
    id
    slug
    active
    metadata
    createdAt
    updatedAt
    translations {
      locale
      name
      description
    }
  }
}`;

const UPDATE_SHIPPING_PROFILE_MUTATION = `
mutation CommerceAdminUpdateShippingProfile($tenantId: UUID!, $id: UUID!, $input: UpdateShippingProfileInputObject!) {
  updateShippingProfile(tenantId: $tenantId, id: $id, input: $input) {
    id
    slug
    active
    metadata
    createdAt
    updatedAt
    translations {
      locale
      name
      description
    }
  }
}`;

const DEACTIVATE_SHIPPING_PROFILE_MUTATION = `
mutation CommerceAdminDeactivateShippingProfile($tenantId: UUID!, $id: UUID!) {
  deactivateShippingProfile(tenantId: $tenantId, id: $id) {
    id
    slug
    active
  }
}`;

const REACTIVATE_SHIPPING_PROFILE_MUTATION = `
mutation CommerceAdminReactivateShippingProfile($tenantId: UUID!, $id: UUID!) {
  reactivateShippingProfile(tenantId: $tenantId, id: $id) {
    id
    slug
    active
  }
}`;

const PREVIEW_CART_PROMOTION_MUTATION = `
mutation CommerceAdminPreviewCartPromotion($tenantId: UUID!, $cartId: UUID!, $input: AdminCartPromotionInput!) {
  previewAdminCartPromotion(tenantId: $tenantId, cartId: $cartId, input: $input) {
    kind
    scope
    lineItemId
    currencyCode
    baseAmount
    adjustmentAmount
    adjustedAmount
  }
}`;

const APPLY_CART_PROMOTION_MUTATION = `
mutation CommerceAdminApplyCartPromotion($tenantId: UUID!, $cartId: UUID!, $input: AdminCartPromotionInput!) {
  applyAdminCartPromotion(tenantId: $tenantId, cartId: $cartId, input: $input) {
    id
    currencyCode
    shippingTotal
    adjustmentTotal
    totalAmount
    adjustments {
      id
      source
      scope
      lineItemId
      amount
      metadata
    }
  }
}`;

const ORDER_CHANGES_QUERY = `
query CommerceAdminOrderChanges($tenantId: UUID!, $filter: OrderChangesFilter) {
  orderChanges(tenantId: $tenantId, filter: $filter) {
    total
    page
    perPage
    hasNext
    items {
      id
      orderId
      changeType
      status
      description
      preview
      metadata
      createdAt
      updatedAt
    }
  }
}`;

const ORDER_CHANGE_QUERY = `
query CommerceAdminOrderChange($tenantId: UUID!, $id: UUID!) {
  orderChange(tenantId: $tenantId, id: $id) {
    id
    orderId
    changeType
    status
    description
    preview
    metadata
    createdAt
    updatedAt
  }
}`;

const CREATE_ORDER_RETURN_DECISION_MUTATION = `
mutation CommerceAdminCreateOrderReturnDecision($tenantId: UUID!, $orderId: UUID!, $input: CreateReturnDecisionInputObject!) {
  createOrderReturnDecision(tenantId: $tenantId, orderId: $orderId, input: $input) {
    action
    metadata
    orderReturn {
      id
      orderId
      status
      resolutionType
      refundId
      orderChangeId
      metadata
      createdAt
      updatedAt
      completedAt
      cancelledAt
    }
    refund {
      id
      status
      currencyCode
      amount
      reason
      metadata
      createdAt
      updatedAt
    }
    orderChange {
      id
      orderId
      changeType
      status
      description
      preview
      metadata
      createdAt
      updatedAt
    }
  }
}`;

const APPLY_ORDER_CHANGE_MUTATION = `
mutation CommerceAdminApplyOrderChange($tenantId: UUID!, $id: UUID!, $input: ApplyOrderChangeInputObject!) {
  applyOrderChange(tenantId: $tenantId, id: $id, input: $input) {
    id
    status
    updatedAt
  }
}`;

const CANCEL_ORDER_CHANGE_MUTATION = `
mutation CommerceAdminCancelOrderChange($tenantId: UUID!, $id: UUID!, $input: CancelOrderChangeInputObject!) {
  cancelOrderChange(tenantId: $tenantId, id: $id, input: $input) {
    id
    status
    updatedAt
  }
}`;

// CLIENT METHODS

export async function listShippingProfiles(
  opts: GqlOpts,
  filter: {
    page?: number;
    perPage?: number;
    active?: boolean;
    search?: string;
  } = {}
): Promise<ShippingProfileList> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage shipping profiles.');
  }

  const response = await opts.graphql<
    { tenantId: string; filter: typeof filter },
    { shippingProfiles: ShippingProfileList }
  >(
    SHIPPING_PROFILES_QUERY,
    { tenantId: opts.tenantId, filter },
    opts.token,
    opts.tenantSlug
  );

  return response.shippingProfiles;
}

export async function getShippingProfile(
  opts: GqlOpts,
  id: string
): Promise<ShippingProfile | null> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage shipping profiles.');
  }

  const response = await opts.graphql<
    { tenantId: string; id: string },
    { shippingProfile: ShippingProfile | null }
  >(
    SHIPPING_PROFILE_QUERY,
    { tenantId: opts.tenantId, id },
    opts.token,
    opts.tenantSlug
  );

  return response.shippingProfile;
}

export async function createShippingProfile(
  opts: GqlOpts,
  input: CreateShippingProfileInput
): Promise<ShippingProfile> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage shipping profiles.');
  }

  const response = await opts.graphql<
    { tenantId: string; input: CreateShippingProfileInput },
    { createShippingProfile: ShippingProfile }
  >(
    CREATE_SHIPPING_PROFILE_MUTATION,
    { tenantId: opts.tenantId, input },
    opts.token,
    opts.tenantSlug
  );

  return response.createShippingProfile;
}

export async function updateShippingProfile(
  opts: GqlOpts,
  id: string,
  input: UpdateShippingProfileInput
): Promise<ShippingProfile> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage shipping profiles.');
  }

  const response = await opts.graphql<
    { tenantId: string; id: string; input: UpdateShippingProfileInput },
    { updateShippingProfile: ShippingProfile }
  >(
    UPDATE_SHIPPING_PROFILE_MUTATION,
    { tenantId: opts.tenantId, id, input },
    opts.token,
    opts.tenantSlug
  );

  return response.updateShippingProfile;
}

export async function deactivateShippingProfile(
  opts: GqlOpts,
  id: string
): Promise<ShippingProfile> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage shipping profiles.');
  }

  const response = await opts.graphql<
    { tenantId: string; id: string },
    { deactivateShippingProfile: ShippingProfile }
  >(
    DEACTIVATE_SHIPPING_PROFILE_MUTATION,
    { tenantId: opts.tenantId, id },
    opts.token,
    opts.tenantSlug
  );

  return response.deactivateShippingProfile;
}

export async function reactivateShippingProfile(
  opts: GqlOpts,
  id: string
): Promise<ShippingProfile> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage shipping profiles.');
  }

  const response = await opts.graphql<
    { tenantId: string; id: string },
    { reactivateShippingProfile: ShippingProfile }
  >(
    REACTIVATE_SHIPPING_PROFILE_MUTATION,
    { tenantId: opts.tenantId, id },
    opts.token,
    opts.tenantSlug
  );

  return response.reactivateShippingProfile;
}

export async function previewCartPromotion(
  opts: GqlOpts,
  cartId: string,
  input: AdminCartPromotionInput
): Promise<CartPromotionPreview> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage promotions.');
  }

  const response = await opts.graphql<
    { tenantId: string; cartId: string; input: AdminCartPromotionInput },
    { previewAdminCartPromotion: CartPromotionPreview }
  >(
    PREVIEW_CART_PROMOTION_MUTATION,
    { tenantId: opts.tenantId, cartId, input },
    opts.token,
    opts.tenantSlug
  );

  return response.previewAdminCartPromotion;
}

export async function applyCartPromotion(
  opts: GqlOpts,
  cartId: string,
  input: AdminCartPromotionInput
): Promise<CartSnapshot> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage promotions.');
  }

  const response = await opts.graphql<
    { tenantId: string; cartId: string; input: AdminCartPromotionInput },
    { applyAdminCartPromotion: CartSnapshot }
  >(
    APPLY_CART_PROMOTION_MUTATION,
    { tenantId: opts.tenantId, cartId, input },
    opts.token,
    opts.tenantSlug
  );

  return response.applyAdminCartPromotion;
}

export async function listOrderChanges(
  opts: GqlOpts,
  filter: {
    page?: number;
    perPage?: number;
    orderId?: string;
    status?: string;
    changeType?: string;
  } = {}
): Promise<OrderChangeList> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage order changes.');
  }

  const response = await opts.graphql<
    { tenantId: string; filter: typeof filter },
    { orderChanges: OrderChangeList }
  >(
    ORDER_CHANGES_QUERY,
    { tenantId: opts.tenantId, filter },
    opts.token,
    opts.tenantSlug
  );

  return response.orderChanges;
}

export async function getOrderChange(
  opts: GqlOpts,
  id: string
): Promise<OrderChange | null> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage order changes.');
  }

  const response = await opts.graphql<
    { tenantId: string; id: string },
    { orderChange: OrderChange | null }
  >(
    ORDER_CHANGE_QUERY,
    { tenantId: opts.tenantId, id },
    opts.token,
    opts.tenantSlug
  );

  return response.orderChange;
}

export async function applyOrderChange(
  opts: GqlOpts,
  id: string,
  metadata?: string | null,
  differenceRefund?: ExchangeDifferenceRefundInput | null
): Promise<OrderChange> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage order changes.');
  }

  const response = await opts.graphql<
    {
      tenantId: string;
      id: string;
      input: {
        metadata?: string | null;
        differenceRefund?: ExchangeDifferenceRefundInput | null;
      };
    },
    { applyOrderChange: OrderChange }
  >(
    APPLY_ORDER_CHANGE_MUTATION,
    { tenantId: opts.tenantId, id, input: { metadata, differenceRefund } },
    opts.token,
    opts.tenantSlug
  );

  return response.applyOrderChange;
}

export async function cancelOrderChange(
  opts: GqlOpts,
  id: string,
  reason?: string | null,
  metadata?: string | null
): Promise<OrderChange> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage order changes.');
  }

  const response = await opts.graphql<
    {
      tenantId: string;
      id: string;
      input: { reason?: string | null; metadata?: string | null };
    },
    { cancelOrderChange: OrderChange }
  >(
    CANCEL_ORDER_CHANGE_MUTATION,
    { tenantId: opts.tenantId, id, input: { reason, metadata } },
    opts.token,
    opts.tenantSlug
  );

  return response.cancelOrderChange;
}

export async function createOrderReturnDecision(
  opts: GqlOpts,
  orderId: string,
  input: CreateReturnDecisionInput
): Promise<ReturnDecisionResponse> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage return decisions.');
  }

  const response = await opts.graphql<
    { tenantId: string; orderId: string; input: CreateReturnDecisionInput },
    { createOrderReturnDecision: ReturnDecisionResponse }
  >(
    CREATE_ORDER_RETURN_DECISION_MUTATION,
    { tenantId: opts.tenantId, orderId, input },
    opts.token,
    opts.tenantSlug
  );

  return response.createOrderReturnDecision;
}
