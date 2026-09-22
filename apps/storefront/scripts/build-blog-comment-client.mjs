#!/usr/bin/env node

import { buildStorefrontWasmClient } from "./build-wasm-client.mjs";

buildStorefrontWasmClient({
  label: "Blog comment",
  logPrefix: "blog-comment-client",
  profileEnv: "RUSTOK_BLOG_COMMENT_PROFILE",
  assetDirEnv: "RUSTOK_BLOG_COMMENT_ASSET_DIR",
  defaultAssetDir: "target/site/assets/blog-comment",
  feature: "blog-comment-island",
  bootstrapSource: "apps/storefront/public/assets/blog-comment-bootstrap.js",
  bootstrapTarget: "blog-comment-bootstrap.js",
  usage: "usage: build-blog-comment-client.mjs [--print-wasm-bindgen-version]",
});
