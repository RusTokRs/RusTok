# Richtext Browser Runtime

The editor is a capability-owned browser runtime, not a content owner. Blog,
Forum, Comments, Pages, Product, Reviews, and future modules keep documents in
their own tables and transport contracts.

The parent creates an iframe with `sandbox="allow-scripts"` and a random nonce
in the URL fragment. The frame can therefore execute the shared editor but has
an opaque origin and receives no cookies, auth tokens, tenant identifiers, API
clients, or persistence access. The initial `window.postMessage` exchange only
transfers a private `MessagePort`; all document traffic then uses that port.

Host adapters must provide:

- an immutable same-origin frame URL from `dist/asset-manifest.json`;
- the server-selected profile identifier;
- the current `RichTextDocument`;
- effective-locale messages already resolved by the host;
- controlled update and error callbacks.

The frame validates structure, profile membership, message size, session id,
and monotonic sequence before applying input. This is a browser boundary and a
UX guard only. Every write must still pass the canonical Rust validator.

Production reads use server-produced `RichTextView.html`; neither Tiptap nor
this frame belongs in anonymous storefront bundles.
