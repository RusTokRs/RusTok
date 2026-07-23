// src/document.ts
function emptyRichTextDocument() {
  return { type: "doc", content: [{ type: "paragraph" }] };
}
function validateRichTextDocument(value, profile) {
  let encoded;
  try {
    encoded = JSON.stringify(value);
  } catch {
    return invalid("document is not serializable");
  }
  if (new TextEncoder().encode(encoded).byteLength > profile.limits.max_json_bytes) {
    return invalid("document exceeds the profile byte limit");
  }
  if (!isRecord(value) || value.type !== "doc" || !Array.isArray(value.content)) {
    return invalid("document root must be a doc with content");
  }
  if (!hasOnlyKeys(value, ["type", "content"])) {
    return invalid("document root contains unsupported fields");
  }
  const stats = { nodes: 0, text: 0, links: 0 };
  for (let index = 0; index < value.content.length; index += 1) {
    const result = validateNode(value.content[index], profile, stats, 1);
    if (!result.valid) return result;
  }
  if (stats.nodes > profile.limits.max_nodes) return invalid("too many nodes");
  if (stats.text > profile.limits.max_text_chars) return invalid("too much text");
  if (stats.links > profile.limits.max_links) return invalid("too many links");
  return { valid: true };
}
function validateNode(value, profile, stats, depth) {
  if (depth > profile.limits.max_depth) return invalid("document is too deep");
  if (!isRecord(value) || typeof value.type !== "string") return invalid("invalid node");
  if (!hasOnlyKeys(value, ["type", "attrs", "content", "marks", "text"])) {
    return invalid("node contains unsupported fields");
  }
  if (!profile.nodes.includes(value.type)) return invalid(`node ${value.type} is not allowed`);
  stats.nodes += 1;
  const node = value;
  if (node.text !== void 0) {
    if (node.type !== "text" || typeof node.text !== "string") return invalid("invalid text node");
    stats.text += [...node.text].length;
  }
  if (!validateNodeAttrs(node, profile)) return invalid(`invalid ${node.type} attributes`);
  if (node.marks !== void 0) {
    if (!Array.isArray(node.marks) || node.marks.length > profile.limits.max_marks_per_node) {
      return invalid("invalid marks");
    }
    for (const mark of node.marks) {
      const result = validateMark(mark, profile, stats);
      if (!result.valid) return result;
    }
  }
  if (node.content !== void 0) {
    if (!Array.isArray(node.content)) return invalid("invalid node content");
    for (const child of node.content) {
      const result = validateNode(child, profile, stats, depth + 1);
      if (!result.valid) return result;
    }
  }
  return { valid: true };
}
function validateNodeAttrs(node, profile) {
  const attrs = node.attrs;
  if (attrs === void 0) return node.type !== "heading";
  if (!isRecord(attrs)) return false;
  if (node.type === "heading") {
    return hasOnlyKeys(attrs, ["level"]) && typeof attrs.level === "number" && profile.heading_levels.includes(attrs.level);
  }
  if (node.type === "orderedList") {
    return hasOnlyKeys(attrs, ["start"]) && (attrs.start === void 0 || Number.isInteger(attrs.start) && Number(attrs.start) >= 1);
  }
  return Object.keys(attrs).length === 0;
}
function validateMark(value, profile, stats) {
  if (!isRecord(value) || typeof value.type !== "string" || !hasOnlyKeys(value, ["type", "attrs"])) {
    return invalid("invalid mark");
  }
  if (!profile.marks.includes(value.type)) return invalid(`mark ${value.type} is not allowed`);
  const mark = value;
  if (mark.type !== "link") {
    return mark.attrs === void 0 || isRecord(mark.attrs) && Object.keys(mark.attrs).length === 0 ? { valid: true } : invalid(`invalid ${mark.type} attributes`);
  }
  if (!isRecord(mark.attrs) || !hasOnlyKeys(mark.attrs, ["href", "target", "rel", "class"])) {
    return invalid("invalid link attributes");
  }
  if (mark.attrs.target != null || mark.attrs.rel != null || mark.attrs.class != null) {
    return invalid("link presentation attributes must be null or absent");
  }
  if (typeof mark.attrs.href !== "string" || !isSafeHref(mark.attrs.href, profile.limits.max_url_bytes)) {
    return invalid("unsafe link");
  }
  stats.links += 1;
  return { valid: true };
}
function isSafeHref(href, maxBytes) {
  if (new TextEncoder().encode(href).byteLength > maxBytes || href.trim() !== href) return false;
  if (href.startsWith("/") && !href.startsWith("//")) return true;
  try {
    const url = new URL(href);
    return (url.protocol === "http:" || url.protocol === "https:" || url.protocol === "mailto:") && !url.username && !url.password;
  } catch {
    return false;
  }
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
var RICH_TEXT_PROTOCOL_REVISION = 1;
var MAX_PROTOCOL_OVERHEAD_BYTES = 16 * 1024;
function createEnvelope(session, sequence, message) {
  return {
    protocol: RICH_TEXT_PROTOCOL,
    revision: RICH_TEXT_PROTOCOL_REVISION,
    session,
    sequence,
    message
  };
}
function isEnvelope(value, session, lastSequence, maxBytes) {
  if (measureMessage(value) > maxBytes || !isRecord3(value)) return false;
  return value.protocol === RICH_TEXT_PROTOCOL && value.revision === RICH_TEXT_PROTOCOL_REVISION && value.session === session && Number.isSafeInteger(value.sequence) && Number(value.sequence) > lastSequence && isRecord3(value.message);
}
function measureMessage(value) {
  try {
    return new TextEncoder().encode(JSON.stringify(value)).byteLength;
  } catch {
    return Number.POSITIVE_INFINITY;
  }
}
function isHandshakeReady(value, nonce) {
  return isRecord3(value) && value.protocol === RICH_TEXT_PROTOCOL && value.revision === RICH_TEXT_PROTOCOL_REVISION && value.type === "ready" && value.nonce === nonce;
}
function isRecord3(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

// src/frame/controller.ts
var RichTextFrameController = class {
  constructor(options) {
    this.options = options;
    this.session = crypto.randomUUID();
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
  maxMessageBytes;
  resolveReady;
  rejectReady;
  cleanupHandshake;
  setDocument(document) {
    this.send({ type: "set_document", payload: { document } });
  }
  setEditable(editable) {
    this.send({ type: "set_editable", payload: { editable } });
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
          revision: RICH_TEXT_PROTOCOL_REVISION,
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
          document: this.options.document,
          messages: this.options.messages,
          editable: this.options.editable ?? true
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
    switch (message.type) {
      case "initialized":
        this.resolveReady();
        break;
      case "document_changed":
      case "document":
        this.options.onDocumentChange(message.payload.document);
        break;
      case "focus_changed":
        this.options.onFocusChange?.(message.payload.focused);
        break;
      case "error":
        this.options.onError?.(message.payload.code, message.payload.message);
        break;
    }
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
    dispose: () => controller.destroy()
  };
}
export {
  MAX_PROTOCOL_OVERHEAD_BYTES,
  RICH_TEXT_MESSAGE_KEYS,
  RICH_TEXT_PROFILES,
  createEnvelope,
  emptyRichTextDocument,
  getRichTextProfile,
  isEnvelope,
  isRichTextMessages,
  isRichTextProfileId,
  mountLeptosRichTextFrame,
  validateRichTextDocument
};
