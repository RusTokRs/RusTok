import { buildStorefrontWasmClient } from "./build-wasm-client.mjs";

buildStorefrontWasmClient({
  label: "Product buyer",
  logPrefix: "product-buyer-client",
  profileEnv: "RUSTOK_PRODUCT_BUYER_PROFILE",
  assetDirEnv: "RUSTOK_PRODUCT_BUYER_ASSET_DIR",
  defaultAssetDir: "target/site/assets/product-buyer",
  feature: "product-buyer-island",
  bootstrapSource: "apps/storefront/public/assets/product-buyer-bootstrap.js",
  bootstrapTarget: "product-buyer-bootstrap.js",
  usage: "usage: build-product-buyer-client.mjs [--print-wasm-bindgen-version]",
});
