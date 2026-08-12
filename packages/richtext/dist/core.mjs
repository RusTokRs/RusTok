// src/document.ts
function emptyRichTextDocument() {
  return { type: "doc", content: [{ type: "paragraph" }] };
}
function richTextDocumentHasText(document) {
  const stack = [...document.content];
  while (stack.length > 0) {
    const node = stack.pop();
    if (node?.text && [...node.text].some((character) => !/\s/u.test(character))) return true;
    if (node?.content) stack.push(...node.content);
  }
  return false;
}
function validateRichTextDocument(value, profile, options = {}) {
  let encoded;
  try {
    encoded = JSON.stringify(value);
  } catch {
    return invalid("document is not serializable");
  }
  if (byteLength(encoded) > profile.limits.max_json_bytes) {
    return invalid("document exceeds the profile byte limit");
  }
  if (!isRecord(value) || value.type !== "doc" || !Array.isArray(value.content)) {
    return invalid("document root must be a doc with content");
  }
  if (!hasOnlyKeys(value, ["type", "content"])) {
    return invalid("document root contains unsupported fields");
  }
  const stats = {
    nodes: 1,
    text: 0,
    links: 0,
    meaningfulText: false
  };
  if (stats.nodes > profile.limits.max_nodes) return invalid("too many nodes");
  for (const child of value.content) {
    if (!isRecord(child) || typeof child.type !== "string" || !isBlock(child.type)) {
      return invalid("document root accepts block nodes only");
    }
    const result = validateNode(child, profile, stats, 2);
    if (!result.valid) return result;
  }
  if (!stats.meaningfulText && !profile.allow_empty && !options.allowEmpty) {
    return invalid("document must contain non-whitespace text");
  }
  return { valid: true };
}
function validateNode(value, profile, stats, depth) {
  if (depth > profile.limits.max_depth) return invalid("document is too deep");
  if (!isRecord(value) || typeof value.type !== "string") return invalid("invalid node");
  if (!hasOnlyKeys(value, ["type", "attrs", "content", "marks", "text"])) {
    return invalid("node contains unsupported fields");
  }
  if (value.type === "doc" || !profile.nodes.includes(value.type)) {
    return invalid(`node ${value.type} is not allowed`);
  }
  stats.nodes += 1;
  if (stats.nodes > profile.limits.max_nodes) return invalid("too many nodes");
  const node = value;
  if (node.type === "text") return validateTextNode(node, profile, stats);
  if (node.text !== void 0) return invalid(`${node.type} cannot contain text directly`);
  if (node.type === "hardBreak") {
    if (!hasEmptyAttrs(node) || !hasNoContent(node)) return invalid("invalid hardBreak structure");
    return validateMarks(node.marks, profile, stats);
  }
  if (!hasNoMarks(node)) return invalid(`${node.type} cannot carry marks`);
  switch (node.type) {
    case "paragraph":
      if (!hasEmptyAttrs(node)) return invalid("invalid paragraph attributes");
      return validateInlineChildren(node, profile, stats, depth);
    case "heading":
      if (!validateHeadingAttrs(node, profile)) return invalid("invalid heading attributes");
      return validateInlineChildren(node, profile, stats, depth);
    case "bulletList":
      if (!hasEmptyAttrs(node)) return invalid("invalid bulletList attributes");
      return validateList(node, profile, stats, depth);
    case "orderedList":
      if (!validateOrderedListAttrs(node)) return invalid("invalid orderedList attributes");
      return validateList(node, profile, stats, depth);
    case "listItem":
      if (!hasEmptyAttrs(node)) return invalid("invalid listItem attributes");
      return validateListItem(node, profile, stats, depth);
    case "blockquote":
      if (!hasEmptyAttrs(node)) return invalid("invalid blockquote attributes");
      return validateBlockChildren(node, profile, stats, depth, true);
    case "codeBlock":
      if (!hasEmptyAttrs(node)) return invalid("invalid codeBlock attributes");
      return validateCodeBlock(node, profile, stats, depth);
    case "horizontalRule":
      return hasEmptyAttrs(node) && hasNoContent(node) ? { valid: true } : invalid("invalid horizontalRule structure");
    default:
      return invalid(`node ${node.type} is not supported`);
  }
}
function validateTextNode(node, profile, stats) {
  if (!hasEmptyAttrs(node) || !hasNoContent(node) || typeof node.text !== "string" || node.text.length === 0) {
    return invalid("invalid text node");
  }
  stats.text += [...node.text].length;
  if (stats.text > profile.limits.max_text_chars) return invalid("too much text");
  stats.meaningfulText ||= [...node.text].some((character) => !/\s/u.test(character));
  return validateMarks(node.marks, profile, stats);
}
function validateMarks(marks, profile, stats) {
  if (marks === void 0) return { valid: true };
  if (!Array.isArray(marks) || marks.length > profile.limits.max_marks_per_node) {
    return invalid("invalid marks");
  }
  const seen = /* @__PURE__ */ new Set();
  for (const mark of marks) {
    if (!isRecord(mark) || typeof mark.type !== "string" || !hasOnlyKeys(mark, ["type", "attrs"])) {
      return invalid("invalid mark");
    }
    if (!profile.marks.includes(mark.type)) return invalid(`mark ${mark.type} is not allowed`);
    if (seen.has(mark.type)) return invalid(`duplicate ${mark.type} mark`);
    seen.add(mark.type);
    if (mark.type !== "link") {
      if (mark.attrs !== void 0 && (!isRecord(mark.attrs) || Object.keys(mark.attrs).length > 0)) {
        return invalid(`invalid ${mark.type} attributes`);
      }
      continue;
    }
    if (!isRecord(mark.attrs) || !hasOnlyKeys(mark.attrs, ["href", "target", "rel", "class"])) {
      return invalid("invalid link attributes");
    }
    if (mark.attrs.target != null || mark.attrs.rel != null || mark.attrs.class != null) {
      return invalid("link presentation attributes must be null or absent");
    }
    if (typeof mark.attrs.href !== "string" || byteLength(mark.attrs.href) > profile.limits.max_attribute_bytes || !isSafeHref(mark.attrs.href, profile.limits.max_url_bytes)) {
      return invalid("unsafe link");
    }
    stats.links += 1;
    if (stats.links > profile.limits.max_links) return invalid("too many links");
  }
  return { valid: true };
}
function validateInlineChildren(node, profile, stats, depth) {
  if (node.content !== void 0 && !Array.isArray(node.content)) return invalid("invalid node content");
  for (const child of node.content ?? []) {
    if (!isRecord(child) || typeof child.type !== "string" || !isInline(child.type)) {
      return invalid(`${node.type} accepts inline nodes only`);
    }
    const result = validateNode(child, profile, stats, depth + 1);
    if (!result.valid) return result;
  }
  return { valid: true };
}
function validateBlockChildren(node, profile, stats, depth, requireChild) {
  if (node.content !== void 0 && !Array.isArray(node.content)) return invalid("invalid node content");
  const children = node.content ?? [];
  if (requireChild && children.length === 0) return invalid(`${node.type} requires content`);
  for (const child of children) {
    if (!isRecord(child) || typeof child.type !== "string" || !isBlock(child.type)) {
      return invalid(`${node.type} accepts block nodes only`);
    }
    const result = validateNode(child, profile, stats, depth + 1);
    if (!result.valid) return result;
  }
  return { valid: true };
}
function validateList(node, profile, stats, depth) {
  if (!Array.isArray(node.content) || node.content.length === 0) return invalid(`${node.type} requires content`);
  for (const child of node.content) {
    if (!isRecord(child) || child.type !== "listItem") return invalid(`${node.type} accepts listItem nodes only`);
    const result = validateNode(child, profile, stats, depth + 1);
    if (!result.valid) return result;
  }
  return { valid: true };
}
function validateListItem(node, profile, stats, depth) {
  if (!Array.isArray(node.content) || node.content[0]?.type !== "paragraph") {
    return invalid("listItem must start with a paragraph");
  }
  return validateBlockChildren(node, profile, stats, depth, true);
}
function validateCodeBlock(node, profile, stats, depth) {
  if (node.content !== void 0 && !Array.isArray(node.content)) return invalid("invalid codeBlock content");
  for (const child of node.content ?? []) {
    if (!isRecord(child) || child.type !== "text" || Array.isArray(child.marks) && child.marks.length > 0) {
      return invalid("codeBlock accepts unmarked text only");
    }
    const result = validateNode(child, profile, stats, depth + 1);
    if (!result.valid) return result;
  }
  return { valid: true };
}
function validateHeadingAttrs(node, profile) {
  return isRecord(node.attrs) && hasOnlyKeys(node.attrs, ["level"]) && typeof node.attrs.level === "number" && Number.isInteger(node.attrs.level) && profile.heading_levels.includes(node.attrs.level);
}
function validateOrderedListAttrs(node) {
  if (node.attrs === void 0) return true;
  if (!isRecord(node.attrs) || !hasOnlyKeys(node.attrs, ["start"])) return false;
  const start = node.attrs.start;
  return start === void 0 || start === null || typeof start === "number" && Number.isInteger(start) && start >= 1 && start <= 1e6;
}
function hasEmptyAttrs(node) {
  return node.attrs === void 0 || isRecord(node.attrs) && Object.keys(node.attrs).length === 0;
}
function hasNoContent(node) {
  return node.content === void 0 || Array.isArray(node.content) && node.content.length === 0;
}
function hasNoMarks(node) {
  return node.marks === void 0 || Array.isArray(node.marks) && node.marks.length === 0;
}
function isBlock(kind) {
  return ["paragraph", "heading", "bulletList", "orderedList", "blockquote", "codeBlock", "horizontalRule"].includes(kind);
}
function isInline(kind) {
  return kind === "text" || kind === "hardBreak";
}
function isSafeHref(href, maxBytes) {
  if (byteLength(href) > maxBytes || href.trim() !== href || href.length === 0 || /[\u0000-\u001f\u007f]/u.test(href) || href.includes("\\") || href.startsWith("//")) {
    return false;
  }
  if (href.startsWith("/")) return true;
  if (href.startsWith("#")) return href.length > 1;
  try {
    const url = new URL(href);
    if (url.protocol === "mailto:") return true;
    return (url.protocol === "http:" || url.protocol === "https:") && !url.username && !url.password && Boolean(url.hostname);
  } catch {
    return false;
  }
}
function byteLength(value) {
  return new TextEncoder().encode(value).byteLength;
}
function hasOnlyKeys(value, allowed) {
  return Object.keys(value).every((key) => allowed.includes(key));
}
function isRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
function invalid(error) {
  return { valid: false, error };
}

// src/messages.ts
var RICH_TEXT_MESSAGE_KEYS = [
  "bold",
  "italic",
  "strike",
  "code",
  "heading",
  "bullet_list",
  "ordered_list",
  "blockquote",
  "code_block",
  "horizontal_rule",
  "link",
  "link_url",
  "apply_link",
  "remove_link",
  "clear_formatting",
  "undo",
  "redo",
  "editor"
];
function isRichTextMessages(value) {
  if (!isRecord2(value)) return false;
  return RICH_TEXT_MESSAGE_KEYS.every(
    (key) => typeof value[key] === "string" && value[key].length > 0
  );
}
function isRecord2(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

// src/authoring.ts
function createRichTextAuthoringContext(input) {
  const locale = canonicalRichTextLocale(input.contentLocale);
  return {
    locale,
    direction: input.direction ?? richTextDirectionForLocale(locale),
    spellcheck: input.spellcheck ?? true
  };
}
function isRichTextAuthoringContext(value) {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return false;
  }
  const context = value;
  return typeof context.locale === "string" && canonicalRichTextLocale(context.locale) === context.locale && (context.direction === "ltr" || context.direction === "rtl" || context.direction === "auto") && typeof context.spellcheck === "boolean";
}
function canonicalRichTextLocale(value) {
  const candidate = value.trim();
  if (!candidate || candidate.length > 64) return "und";
  try {
    return Intl.getCanonicalLocales(candidate)[0] ?? "und";
  } catch {
    return "und";
  }
}
function richTextDirectionForLocale(locale) {
  if (locale === "und") return "auto";
  try {
    const localeInfo = new Intl.Locale(locale);
    const direction = localeInfo.textInfo?.direction ?? localeInfo.getTextInfo?.().direction;
    return direction === "ltr" || direction === "rtl" ? direction : "auto";
  } catch {
    return "auto";
  }
}

// src/generated/profiles.json
var profiles_default = [
  {
    id: "article",
    nodes: ["doc", "paragraph", "heading", "bulletList", "orderedList", "listItem", "blockquote", "codeBlock", "horizontalRule", "hardBreak", "text"],
    marks: ["bold", "italic", "strike", "code", "link"],
    heading_levels: [2, 3, 4],
    allow_empty: false,
    external_link_rel: "noopener noreferrer",
    limits: { max_json_bytes: 524288, max_depth: 16, max_nodes: 4096, max_text_chars: 2e5, max_marks_per_node: 8, max_links: 512, max_attribute_bytes: 2048, max_url_bytes: 2048 }
  },
  {
    id: "discussion",
    nodes: ["doc", "paragraph", "heading", "bulletList", "orderedList", "listItem", "blockquote", "codeBlock", "horizontalRule", "hardBreak", "text"],
    marks: ["bold", "italic", "strike", "code", "link"],
    heading_levels: [2, 3, 4],
    allow_empty: false,
    external_link_rel: "noopener noreferrer nofollow ugc",
    limits: { max_json_bytes: 262144, max_depth: 14, max_nodes: 2048, max_text_chars: 1e5, max_marks_per_node: 8, max_links: 256, max_attribute_bytes: 2048, max_url_bytes: 2048 }
  },
  {
    id: "comment",
    nodes: ["doc", "paragraph", "bulletList", "orderedList", "listItem", "blockquote", "hardBreak", "text"],
    marks: ["bold", "italic", "strike", "code", "link"],
    heading_levels: [],
    allow_empty: false,
    external_link_rel: "noopener noreferrer nofollow ugc",
    limits: { max_json_bytes: 65536, max_depth: 10, max_nodes: 512, max_text_chars: 2e4, max_marks_per_node: 6, max_links: 32, max_attribute_bytes: 1024, max_url_bytes: 1024 }
  }
];

// src/generated/profiles.ts
var RICH_TEXT_PROFILES = profiles_default;

// src/profiles.ts
var profiles = new Map(
  RICH_TEXT_PROFILES.map((profile) => [profile.id, profile])
);
function getRichTextProfile(id) {
  const profile = profiles.get(id);
  if (!profile) throw new Error(`Unknown richtext profile: ${id}`);
  return profile;
}
function isRichTextProfileId(value) {
  return typeof value === "string" && profiles.has(value);
}

// src/protocol.ts
var RICH_TEXT_PROTOCOL = "rustok.richtext";
var MAX_PROTOCOL_OVERHEAD_BYTES = 16 * 1024;
function createEnvelope(session, sequence, message) {
  return {
    protocol: RICH_TEXT_PROTOCOL,
    session,
    sequence,
    message
  };
}
function isEnvelope(value, session, lastSequence, maxBytes) {
  if (measureMessage(value) > maxBytes || !isRecord3(value)) return false;
  return value.protocol === RICH_TEXT_PROTOCOL && value.session === session && Number.isSafeInteger(value.sequence) && Number(value.sequence) > lastSequence && isRecord3(value.message);
}
function measureMessage(value) {
  try {
    return new TextEncoder().encode(JSON.stringify(value)).byteLength;
  } catch {
    return Number.POSITIVE_INFINITY;
  }
}
function isHandshakeReady(value, nonce) {
  return isRecord3(value) && value.protocol === RICH_TEXT_PROTOCOL && value.type === "ready" && value.nonce === nonce;
}
function isRecord3(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

// src/frame/controller.ts
var RichTextFrameController = class {
  constructor(options) {
    this.options = options;
    this.session = crypto.randomUUID();
    this.document = options.document;
    this.editable = options.editable ?? true;
    this.authoringContext = createRichTextAuthoringContext({
      contentLocale: options.contentLocale,
      direction: options.direction,
      spellcheck: options.spellcheck
    });
    this.maxMessageBytes = getRichTextProfile(options.profile).limits.max_json_bytes + MAX_PROTOCOL_OVERHEAD_BYTES;
    this.ready = new Promise((resolve, reject) => {
      this.resolveReady = resolve;
      this.rejectReady = reject;
    });
    this.connect();
  }
  session;
  ready;
  port;
  outboundSequence = 0;
  inboundSequence = 0;
  destroyed = false;
  document;
  editable;
  authoringContext;
  maxMessageBytes;
  resolveReady;
  rejectReady;
  cleanupHandshake;
  setDocument(document) {
    const validation = validateRichTextDocument(
      document,
      getRichTextProfile(this.options.profile),
      { allowEmpty: true }
    );
    if (!validation.valid) {
      this.options.onError?.(
        "invalid_document",
        validation.error ?? "The host supplied an invalid richtext document."
      );
      return;
    }
    this.document = document;
    this.send({ type: "set_document", payload: { document } });
  }
  setEditable(editable) {
    this.editable = editable;
    this.send({ type: "set_editable", payload: { editable } });
  }
  setAuthoringContext(input) {
    this.authoringContext = createRichTextAuthoringContext(input);
    this.send({
      type: "set_authoring_context",
      payload: this.authoringContext
    });
  }
  focus() {
    this.send({ type: "focus", payload: {} });
  }
  requestDocument() {
    this.send({ type: "request_document", payload: {} });
  }
  destroy() {
    if (this.destroyed) return;
    if (this.port) this.send({ type: "destroy", payload: {} });
    this.destroyed = true;
    this.cleanupHandshake?.();
    this.port?.close();
    this.port = void 0;
  }
  connect() {
    const nonce = crypto.randomUUID();
    const url = new URL(this.options.frameUrl, window.location.href);
    url.hash = new URLSearchParams({ nonce }).toString();
    const timeout = window.setTimeout(() => {
      this.cleanupHandshake?.();
      this.rejectReady(new Error("Richtext frame handshake timed out"));
    }, this.options.timeoutMs ?? 1e4);
    const onMessage = (event) => {
      if (event.source !== this.options.iframe.contentWindow || !isHandshakeReady(event.data, nonce)) {
        return;
      }
      window.clearTimeout(timeout);
      window.removeEventListener("message", onMessage);
      const channel = new MessageChannel();
      this.port = channel.port1;
      this.port.onmessage = (portEvent) => this.receive(portEvent.data);
      this.port.start();
      this.options.iframe.contentWindow?.postMessage(
        {
          protocol: RICH_TEXT_PROTOCOL,
          type: "connect",
          nonce,
          session: this.session
        },
        "*",
        [channel.port2]
      );
      this.send({
        type: "initialize",
        payload: {
          profile: this.options.profile,
          document: this.document,
          messages: this.options.messages,
          authoring_context: this.authoringContext,
          editable: this.editable
        }
      });
    };
    this.cleanupHandshake = () => {
      window.clearTimeout(timeout);
      window.removeEventListener("message", onMessage);
    };
    window.addEventListener("message", onMessage);
    this.options.iframe.src = url.toString();
  }
  send(message) {
    if (this.destroyed || !this.port) return;
    this.outboundSequence += 1;
    this.port.postMessage(
      createEnvelope(this.session, this.outboundSequence, message)
    );
  }
  receive(value) {
    if (!isEnvelope(
      value,
      this.session,
      this.inboundSequence,
      this.maxMessageBytes
    )) {
      this.options.onError?.("invalid_message", "The editor frame returned an invalid message.");
      return;
    }
    this.inboundSequence = value.sequence;
    const message = value.message;
    if (!isRecord3(message) || typeof message.type !== "string" || !isRecord3(message.payload)) {
      this.options.onError?.("invalid_message", "The editor frame returned an invalid message.");
      return;
    }
    switch (message.type) {
      case "initialized": {
        if (!this.acceptDocument(message.payload.document)) return;
        this.resolveReady();
        break;
      }
      case "document_changed":
      case "document": {
        const document = this.acceptDocument(message.payload.document);
        if (!document) return;
        this.options.onDocumentChange(document);
        break;
      }
      case "focus_changed": {
        if (typeof message.payload.focused !== "boolean") {
          this.options.onError?.("invalid_message", "The editor frame returned an invalid focus state.");
          return;
        }
        this.options.onFocusChange?.(message.payload.focused);
        break;
      }
      case "error": {
        if (typeof message.payload.code !== "string" || typeof message.payload.message !== "string") {
          this.options.onError?.("invalid_message", "The editor frame returned an invalid error message.");
          return;
        }
        this.options.onError?.(message.payload.code, message.payload.message);
        break;
      }
      default:
        this.options.onError?.("invalid_message", "The editor frame returned an unsupported message.");
    }
  }
  acceptDocument(value) {
    const validation = validateRichTextDocument(
      value,
      getRichTextProfile(this.options.profile),
      { allowEmpty: true }
    );
    if (!validation.valid) {
      this.options.onError?.(
        "invalid_document",
        validation.error ?? "The editor frame returned an invalid richtext document."
      );
      return void 0;
    }
    return value;
  }
};
function connectRichTextFrame(options) {
  return new RichTextFrameController(options);
}

// src/leptos.ts
function mountLeptosRichTextFrame(iframe, options) {
  const controller = connectRichTextFrame({ iframe, ...options });
  return {
    controller,
    setDocument: (document) => controller.setDocument(document),
    setAuthoringContext: (input) => controller.setAuthoringContext(input),
    setEditable: (editable) => controller.setEditable(editable),
    dispose: () => controller.destroy()
  };
}
export {
  MAX_PROTOCOL_OVERHEAD_BYTES,
  RICH_TEXT_MESSAGE_KEYS,
  RICH_TEXT_PROFILES,
  canonicalRichTextLocale,
  createEnvelope,
  createRichTextAuthoringContext,
  emptyRichTextDocument,
  getRichTextProfile,
  isEnvelope,
  isRichTextAuthoringContext,
  isRichTextMessages,
  isRichTextProfileId,
  mountLeptosRichTextFrame,
  richTextDirectionForLocale,
  richTextDocumentHasText,
  validateRichTextDocument
};
