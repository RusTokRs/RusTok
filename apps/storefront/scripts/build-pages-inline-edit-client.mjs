#!/usr/bin/env node

import { buildStorefrontWasmClient } from "./build-wasm-client.mjs";

buildStorefrontWasmClient({
  label: "Pages inline edit",
  logPrefix: "pages-inline-edit-client",
  profileEnv: "RUSTOK_PAGES_INLINE_EDIT_PROFILE",
  assetDirEnv: "RUSTOK_PAGES_INLINE_EDIT_ASSET_DIR",
  defaultAssetDir: "target/site/assets/pages-inline-edit",
  feature: "pages-inline-edit-hydrate",
  bootstrapSource: "apps/storefront/public/assets/pages-inline-edit-bootstrap.js",
  bootstrapTarget: "pages-inline-edit-bootstrap.js",
  usage: "usage: build-pages-inline-edit-client.mjs [--print-wasm-bindgen-version]",
});
