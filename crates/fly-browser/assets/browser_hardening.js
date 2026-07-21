(() => {
  const api = globalThis.FlyBrowser;
  const Adapter = api?.FlyBrowserAdapter;
  if (!Adapter || Adapter.prototype.__flyResourceGuardInstalled) return;

  const DEFAULT_MAX_MESSAGE_BYTES = 1024 * 1024;
  const DEFAULT_MAX_GEOMETRY_COMPONENTS = 4096;
  const DEFAULT_MAX_PENDING_INTENT_REQUESTS = 8;
  const DEFAULT_INTENT_REQUEST_TIMEOUT_MS = 30_000;
  const PENDING_INTENT_LIMIT_CODE = "PENDING_INTENT_LIMIT";
  const INTENT_REQUEST_TIMEOUT_CODE = "INTENT_REQUEST_TIMEOUT";
  const INTENT_REQUEST_ABORTED_CODE = "INTENT_REQUEST_ABORTED";
  const INTENT_ABORT_KIND = Object.freeze({
    EXTERNAL: "external",
    TIMEOUT: "timeout",
    ADAPTER_STOP: "adapter_stop",
  });
  const ADAPTER_KEY = Symbol.for("fly.browser.adapter");
  const PROBLEM_REPORTER_KEY = Symbol.for("fly.browser.problem.reporter");
  const PENDING_INTENT_CONTROLLERS_KEY = Symbol.for(
    "fly.browser.pending.intent.controllers",
  );
  const RESOURCE_STATUS_SELECTOR =
    '[data-fly-browser-status="resource-limit"]';
  const PROBLEM_STATUS_SELECTOR = '[data-fly-browser-status="problem"]';
  const ROOT_SELECTOR = "[data-fly-browser-root]";

  /**
   * @typedef {"external" | "timeout" | "adapter_stop"} IntentAbortKind
   * @typedef {{ signal?: AbortSignal }} IntentTransportOptions
   */

  const isObject = (value) =>
    value !== null && typeof value === "object" && !Array.isArray(value);

  const isAbortSignal = (value) =>
    typeof AbortSignal === "function" && value instanceof AbortSignal;

  const normalizedTransportOptions = (value) => {
    const transport = isObject(value) ? value : {};
    return {
      ...transport,
      signal: isAbortSignal(transport.signal) ? transport.signal : undefined,
    };
  };

  const boundedPositiveInteger = (value, fallback) => {
    const parsed = Number(value);
    return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : fallback;
  };

  const utf8ByteLength = (value) => {
    if (typeof TextEncoder === "function") {
      return new TextEncoder().encode(value).byteLength;
    }
    return typeof Blob === "function" ? new Blob([value]).size : value.length;
  };

  const limitFor = (adapter, optionName, dataName, fallback) =>
    boundedPositiveInteger(
      adapter.options?.[optionName] ?? adapter.root.dataset[dataName],
      fallback,
    );

  const visuallyHiddenStyle =
    "position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0";

  const ensureResourceStatus = (adapter) => {
    let status = adapter.root.querySelector(RESOURCE_STATUS_SELECTOR);
    if (status) return status;
    status = document.createElement("p");
    status.dataset.flyBrowserStatus = "resource-limit";
    status.setAttribute("role", "status");
    status.setAttribute("aria-live", "polite");
    status.setAttribute("aria-atomic", "true");
    status.style.cssText = visuallyHiddenStyle;
    adapter.root.appendChild(status);
    return status;
  };

  const ensureProblemStatus = (adapter) => {
    let status = adapter.root.querySelector(PROBLEM_STATUS_SELECTOR);
    if (status) return status;
    status = document.createElement("p");
    status.dataset.flyBrowserStatus = "problem";
    status.setAttribute("role", "alert");
    status.setAttribute("aria-live", "assertive");
    status.setAttribute("aria-atomic", "true");
    status.style.cssText = visuallyHiddenStyle;
    adapter.root.appendChild(status);
    return status;
  };

  const normalizedStringList = (value) =>
    Array.isArray(value)
      ? [...new Set(value.filter((item) => typeof item === "string" && item))]
      : [];

  const normalizedProblem = (adapter, detail, fallbackCode) => {
    const result = isObject(detail?.result) ? detail.result : {};
    const request = isObject(detail?.request) ? detail.request : {};
    const status = Number.isSafeInteger(detail?.status) ? detail.status : 0;
    const code =
      typeof result.code === "string" && result.code
        ? result.code
        : fallbackCode || (status > 0 ? `HTTP_${status}` : "BROWSER_REQUEST_FAILED");
    const error =
      typeof result.error === "string" && result.error
        ? result.error
        : typeof detail?.error === "string" && detail.error
          ? detail.error
          : status > 0
            ? `Editor action failed with status ${status}.`
            : "Editor action failed.";
    return {
      status,
      code,
      error,
      intent:
        typeof result.intent === "string" && result.intent
          ? result.intent
          : typeof request.intent === "string" && request.intent
            ? request.intent
            : null,
      capability:
        typeof result.capability === "string" && result.capability
          ? result.capability
          : null,
      required: normalizedStringList(result.required),
      missing: normalizedStringList(result.missing),
      instanceId: adapter.instanceId,
      pageId: adapter.pageId,
    };
  };

  const clearBrowserProblem = (adapter) => {
    delete adapter.root.dataset.flyBrowserProblem;
    adapter.root.querySelector(PROBLEM_STATUS_SELECTOR)?.remove();
  };

  const reportBrowserProblem = (adapter, detail, fallbackCode) => {
    const problem = normalizedProblem(adapter, detail, fallbackCode);
    adapter.root.dataset.flyBrowserProblem = problem.code;
    adapter.root.dispatchEvent(
      new CustomEvent("fly:browser-problem", {
        bubbles: true,
        detail: problem,
      }),
    );
    const status = ensureProblemStatus(adapter);
    status.textContent = "";
    queueMicrotask(() => {
      status.textContent = problem.error;
    });
  };

  const pendingIntentControllers = (adapter) => {
    let controllers = adapter[PENDING_INTENT_CONTROLLERS_KEY];
    if (controllers instanceof Map) return controllers;
    controllers = new Map();
    adapter[PENDING_INTENT_CONTROLLERS_KEY] = controllers;
    return controllers;
  };

  const forwardedAbortError = (signal) =>
    signal.reason === undefined
      ? "Editor action cancelled."
      : String(signal.reason);

  const forwardAbortSignal = (signal, controller, record) => {
    if (!isAbortSignal(signal)) return null;
    const abort = () => {
      record.abortCode = INTENT_REQUEST_ABORTED_CODE;
      record.abortKind = INTENT_ABORT_KIND.EXTERNAL;
      record.error = forwardedAbortError(signal);
      controller.abort(signal.reason);
    };
    if (signal.aborted) {
      abort();
      return null;
    }
    signal.addEventListener("abort", abort, { once: true });
    return () => signal.removeEventListener("abort", abort);
  };

  const intentName = (input) => {
    const request = isObject(input) ? input : {};
    const message = isObject(request.message) ? request.message : {};
    const intent = String(
      request.intent || request.type || message.type || "",
    ).trim().toLowerCase();
    return intent || null;
  };

  const pendingIntentRecordForGeneration = (adapter, requestGeneration) => {
    if (!Number.isSafeInteger(requestGeneration)) return null;
    const controllers = adapter[PENDING_INTENT_CONTROLLERS_KEY];
    if (!(controllers instanceof Map)) return null;
    for (const record of controllers.values()) {
      if (record?.requestGeneration === requestGeneration) return record;
    }
    return null;
  };

  const reportIntentAborted = (adapter, record, detail = {}) => {
    const current = detail.current !== false;
    const aborted = {
      code: record.abortCode || INTENT_REQUEST_ABORTED_CODE,
      kind: record.abortKind || INTENT_ABORT_KIND.EXTERNAL,
      error:
        record.error ||
        (typeof detail.error === "string" && detail.error
          ? detail.error
          : "Editor action cancelled."),
      intent: record.intent,
      request: isObject(detail.request) ? detail.request : record.request,
      requestGeneration: Number.isSafeInteger(detail.requestGeneration)
        ? detail.requestGeneration
        : record.requestGeneration,
      current,
      instanceId: adapter.instanceId,
      pageId: adapter.pageId,
    };
    adapter.root.dispatchEvent(
      new CustomEvent("fly:browser-intent-aborted", {
        bubbles: true,
        detail: aborted,
      }),
    );
    return aborted;
  };

  const rejectPendingIntent = (adapter, input, limit, observed) => {
    const request = isObject(input) ? input : {};
    const message =
      adapter.options?.pendingIntentLimitMessage ||
      adapter.root.dataset.flyPendingIntentLimitMessage ||
      "Editor action limit reached.";
    const error = `${message} ${observed}/${limit}.`;
    const intent = intentName(request);
    const detail = {
      code: PENDING_INTENT_LIMIT_CODE,
      error,
      intent,
      limit,
      observed,
      instanceId: adapter.instanceId,
      pageId: adapter.pageId,
    };
    adapter.root.dispatchEvent(
      new CustomEvent("fly:browser-intent-rejected", {
        bubbles: true,
        detail,
      }),
    );
    reportBrowserProblem(
      adapter,
      {
        status: 0,
        result: {
          code: detail.code,
          error: detail.error,
          intent: detail.intent,
        },
        request,
      },
      PENDING_INTENT_LIMIT_CODE,
    );
    return null;
  };

  const reportIntentTimeout = (adapter, input, record) => {
    const timeoutMs = record.timeoutMs;
    const message =
      adapter.options?.intentRequestTimeoutMessage ||
      adapter.root.dataset.flyIntentRequestTimeoutMessage ||
      "Editor action timed out";
    const error = `${message} after ${timeoutMs} ms.`;
    const detail = {
      code: INTENT_REQUEST_TIMEOUT_CODE,
      error,
      intent: record.intent,
      timeoutMs,
      requestGeneration: record.requestGeneration,
      current:
        record.requestGeneration === adapter.latestIntentRequestGeneration,
      instanceId: adapter.instanceId,
      pageId: adapter.pageId,
    };
    record.abortCode = INTENT_REQUEST_TIMEOUT_CODE;
    record.abortKind = INTENT_ABORT_KIND.TIMEOUT;
    record.timedOut = true;
    record.error = error;
    adapter.root.dispatchEvent(
      new CustomEvent("fly:browser-intent-timeout", {
        bubbles: true,
        detail,
      }),
    );
    return detail;
  };

  const installProblemReporter = (adapter) => {
    if (!adapter || adapter[PROBLEM_REPORTER_KEY]) return adapter;
    adapter[PROBLEM_REPORTER_KEY] = true;
    const signal = adapter.abortController?.signal;
    const options = signal ? { signal } : undefined;
    adapter.root.addEventListener(
      "fly:browser-intent-response",
      (event) => {
        if (event.detail?.current === false) return;
        if (event.detail?.ok === true) {
          clearBrowserProblem(adapter);
        } else {
          reportBrowserProblem(adapter, event.detail, null);
        }
      },
      options,
    );
    adapter.root.addEventListener(
      "fly:browser-error",
      (event) => {
        const record = pendingIntentRecordForGeneration(
          adapter,
          event.detail?.requestGeneration,
        );
        if (record?.controller?.signal.aborted) {
          const aborted = reportIntentAborted(adapter, record, event.detail);
          if (
            aborted.current &&
            aborted.kind === INTENT_ABORT_KIND.TIMEOUT
          ) {
            reportBrowserProblem(
              adapter,
              {
                status: 0,
                result: {
                  code: aborted.code,
                  error: aborted.error,
                  intent: aborted.intent,
                },
                request: aborted.request,
              },
              INTENT_REQUEST_TIMEOUT_CODE,
            );
          }
          return;
        }
        if (event.detail?.current === false) return;
        reportBrowserProblem(
          adapter,
          {
            status: 0,
            error: event.detail?.error,
            request: event.detail?.request,
          },
          "NETWORK_ERROR",
        );
      },
      options,
    );
    return adapter;
  };

  const normalizedLimit = (detail, fallbackKind) => ({
    kind:
      typeof detail?.kind === "string" && detail.kind
        ? detail.kind
        : fallbackKind,
    limit: boundedPositiveInteger(detail?.limit, 1),
    observed: Math.max(0, Number(detail?.observed) || 0),
  });

  const reportResourceLimit = (adapter, detail, fallbackKind) => {
    const resourceLimit = normalizedLimit(detail, fallbackKind);
    adapter.root.dataset.flyResourceLimited = resourceLimit.kind;
    adapter.root.dispatchEvent(
      new CustomEvent("fly:browser-resource-limit", {
        bubbles: true,
        detail: {
          ...resourceLimit,
          instanceId: adapter.instanceId,
          pageId: adapter.pageId,
        },
      }),
    );
    const message =
      adapter.options?.resourceLimitMessage ||
      adapter.root.dataset.flyResourceLimitMessage ||
      "Editor canvas resource limit reached.";
    const status = ensureResourceStatus(adapter);
    status.textContent = "";
    queueMicrotask(() => {
      status.textContent = `${message} ${resourceLimit.kind}: ${resourceLimit.observed}/${resourceLimit.limit}.`;
    });
  };

  const originalStart = Adapter.prototype.start;
  Adapter.prototype.start = function startWithProblemReporter() {
    const result = originalStart.call(this);
    installProblemReporter(this);
    return result;
  };

  const originalPostIntent = Adapter.prototype.postIntent;
  /**
   * @param {unknown} input
   * @param {IntentTransportOptions} requestOptions
   */
  Adapter.prototype.postIntent = function postIntentWithPendingLimit(
    input,
    requestOptions = {},
  ) {
    const transport = normalizedTransportOptions(requestOptions);
    if (!this.intentEndpoint) {
      return originalPostIntent.call(this, input, transport);
    }
    const intent = intentName(input);
    if (!intent) return originalPostIntent.call(this, input, transport);
    const controllers = pendingIntentControllers(this);
    const limit = limitFor(
      this,
      "maxPendingIntentRequests",
      "flyMaxPendingIntentRequests",
      DEFAULT_MAX_PENDING_INTENT_REQUESTS,
    );
    const observed = controllers.size + 1;
    if (observed > limit) {
      return Promise.resolve(rejectPendingIntent(this, input, limit, observed));
    }

    const timeoutMs = limitFor(
      this,
      "intentRequestTimeoutMs",
      "flyIntentRequestTimeoutMs",
      DEFAULT_INTENT_REQUEST_TIMEOUT_MS,
    );
    const controller = new AbortController();
    const requestKey = Symbol("fly.browser.intent.request");
    const record = {
      abortCode: null,
      abortKind: null,
      controller,
      error: null,
      intent,
      request: isObject(input) ? input : {},
      requestGeneration: null,
      timedOut: false,
      timeoutId: null,
      timeoutMs,
    };
    const releaseForwardedAbort = forwardAbortSignal(
      transport.signal,
      controller,
      record,
    );
    controllers.set(requestKey, record);
    let pending;
    try {
      pending = originalPostIntent.call(this, input, {
        ...transport,
        signal: controller.signal,
      });
      record.requestGeneration = Number.isSafeInteger(
        this.intentRequestGeneration,
      )
        ? this.intentRequestGeneration
        : null;
      if (!controller.signal.aborted) {
        record.timeoutId = globalThis.setTimeout(() => {
          if (!controllers.has(requestKey) || controller.signal.aborted) return;
          reportIntentTimeout(this, input, record);
          controller.abort(record.error);
        }, timeoutMs);
      }
    } catch (error) {
      releaseForwardedAbort?.();
      controllers.delete(requestKey);
      throw error;
    }
    return Promise.resolve(pending).finally(() => {
      if (record.timeoutId !== null) globalThis.clearTimeout(record.timeoutId);
      releaseForwardedAbort?.();
      controllers.delete(requestKey);
    });
  };

  const originalOnMessage = Adapter.prototype.onMessage;
  Adapter.prototype.onMessage = function onMessageWithResourceLimit(event) {
    if (
      event.source === this.iframe.contentWindow &&
      event.origin === this.expectedOrigin &&
      typeof event.data === "string"
    ) {
      const limit = limitFor(
        this,
        "maxMessageBytes",
        "flyMaxMessageBytes",
        DEFAULT_MAX_MESSAGE_BYTES,
      );
      const observed = utf8ByteLength(event.data);
      if (observed > limit) {
        reportResourceLimit(
          this,
          { kind: "message_bytes", limit, observed },
          "message_bytes",
        );
        return;
      }
    }
    return originalOnMessage.call(this, event);
  };

  const originalApplyBrowserMessage = Adapter.prototype.applyBrowserMessage;
  Adapter.prototype.applyBrowserMessage = function applyBrowserMessageWithResourceLimit(
    message,
  ) {
    if (message?.type === "geometry_snapshot") {
      if (isObject(message.resource_limit)) {
        const result = originalApplyBrowserMessage.call(this, message);
        reportResourceLimit(
          this,
          message.resource_limit,
          "geometry_components",
        );
        return result;
      }
      const components = Array.isArray(message.components)
        ? message.components
        : [];
      const limit = limitFor(
        this,
        "maxGeometryComponents",
        "flyMaxGeometryComponents",
        DEFAULT_MAX_GEOMETRY_COMPONENTS,
      );
      if (components.length > limit) {
        this.geometry.clear();
        this.drawSelection();
        this.drawDrop();
        reportResourceLimit(
          this,
          {
            kind: "geometry_components",
            limit,
            observed: components.length,
          },
          "geometry_components",
        );
        return;
      }
      if (this.root.dataset.flyResourceLimited === "geometry_components") {
        delete this.root.dataset.flyResourceLimited;
      }
    }
    return originalApplyBrowserMessage.call(this, message);
  };

  const originalStop = Adapter.prototype.stop;
  Adapter.prototype.stop = function stopWithBrowserHardeningCleanup() {
    const controllers = this[PENDING_INTENT_CONTROLLERS_KEY];
    if (controllers instanceof Map) {
      for (const record of controllers.values()) {
        if (record?.timeoutId !== null) globalThis.clearTimeout(record.timeoutId);
        if (record?.controller && !record.controller.signal.aborted) {
          record.abortCode = INTENT_REQUEST_ABORTED_CODE;
          record.abortKind = INTENT_ABORT_KIND.ADAPTER_STOP;
          record.error = "Editor action cancelled because the adapter stopped.";
          reportIntentAborted(this, record, {
            current: false,
            request: record.request,
            requestGeneration: record.requestGeneration,
          });
          record.controller.abort(record.error);
        }
      }
      controllers.clear();
    }
    const result = originalStop.call(this);
    delete this[PENDING_INTENT_CONTROLLERS_KEY];
    delete this.root.dataset.flyResourceLimited;
    this.root.querySelector(RESOURCE_STATUS_SELECTOR)?.remove();
    clearBrowserProblem(this);
    return result;
  };

  for (const root of document.querySelectorAll(ROOT_SELECTOR)) {
    installProblemReporter(root[ADAPTER_KEY]);
  }

  Object.defineProperty(Adapter.prototype, "__flyResourceGuardInstalled", {
    configurable: false,
    enumerable: false,
    value: true,
    writable: false,
  });
})();
