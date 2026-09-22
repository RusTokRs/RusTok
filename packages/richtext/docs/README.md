# Richtext Browser Runtime

The editor is a capability-owned browser runtime, not a content owner. Blog,
Forum, Comments, and explicitly opted-in future consumers keep documents in
their own tables and transport contracts. Pages body remains an independent
Page Builder/Fly document.

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
- the owner-selected content locale and current editable/read-only state;
- controlled update and error callbacks.

Next hosts expose the shared frame and immutable assets through the server-only
`@rustok/richtext/next` route helpers. Host routes stay as thin framework
entrypoints and do not copy filesystem lookup, allowlisting, CSP, or cache
policy.

The React adapter calls the controller directly. A Leptos module-owned UI
package renders the iframe in its `ui/leptos.rs`, invokes
`mountLeptosRichTextFrame` from a browser-only hydration `Effect`, synchronizes
controlled document changes through the returned handle, and calls `dispose`
from `on_cleanup`. Both adapters update locale/direction/spellcheck and
editable/read-only state without remounting the editor. Invalid or temporarily
empty content locale becomes neutral `und`, never a package-selected language.
SSR only emits iframe markup; it does not execute
the WASM bridge. That Rust package remains a transport/UI binding; it does not
copy the editor schema, toolbar, or frame protocol.

The unversioned frame protocol validates structure, the complete current
profile grammar, message size, session id, and monotonic sequence before
applying input. The host validates frame-produced documents again before
forwarding them to an owner form. This is a browser boundary and a UX guard
only. Every write must still pass the canonical Rust validator.

The Chromium frame harness verifies opaque-origin isolation, authoring-context
changes (`ar/rtl` to `fr/ltr`), spellcheck changes, dynamic read-only and
editable transitions, controlled document updates, and cleanup-safe private
channel behavior. Firefox, WebKit, full owner-mounted save/reload, accessibility
and IME evidence remain release work.

Production reads use server-produced `RichTextView.html` through the shared
React `@rustok/richtext/view` or Leptos `RichTextHtml` boundary. Those
components accept the typed projection rather than raw caller HTML, attach the
owner content locale with automatic direction, and do not import Tiptap or the
frame runtime. Neither Tiptap nor this frame belongs in anonymous storefront
bundles.
