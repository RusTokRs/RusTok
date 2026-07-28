const SAFE_QUERY_SOURCE_PATHS = [
  'crates/rustok-commerce/src/graphql/safe_query.rs',
  'crates/rustok-commerce/src/graphql/safe_query/query_error_boundary.rs',
  'crates/rustok-commerce/src/graphql/safe_query/source.rs',
  'crates/rustok-commerce/src/graphql/safe_query/source/rustok_fulfillment_shim.rs',
  'crates/rustok-commerce/src/graphql/safe_query/source/fulfillment_query_service.rs',
  'crates/rustok-commerce/src/graphql/safe_query/source/fulfillment_query_boundary.rs',
];

const FULFILLMENT_SHIM_SOURCE_PATHS = SAFE_QUERY_SOURCE_PATHS.slice(3);

const readCombined = (read, paths) => paths.map((file) => read(file)).join('\n');

export const readCommerceSafeQuerySource = (read) =>
  readCombined(read, SAFE_QUERY_SOURCE_PATHS);

export const readCommerceFulfillmentQueryShimSource = (read) =>
  readCombined(read, FULFILLMENT_SHIM_SOURCE_PATHS);
