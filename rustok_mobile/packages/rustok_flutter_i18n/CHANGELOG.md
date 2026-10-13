# Changelog

## Unreleased

* Inherit full core CLDR likely-script inference and RTL metadata for scope direction; add an unexecuted regional-direction/explicit-override widget regression. Regional lazy loading can request additional script candidates.

* Inherit core typed-selector/nested-function/currency/plural corrections; document numeric-string compatibility and raw-number plural policy. Add an unexecuted context.t rounded-selector regression.

* Re-export corrected date formatting, including en-GB and narrow text widths, and add a `context.t` date-rendering regression. No automatic changes to Flutter framework locale/delegate configuration.

* Re-export the core's strict Accept-Language diagnostics and corrected exclusion-aware negotiation; locale byte bounds and extension serialization are consistent. Host locale selection remains explicit, with no new automatic provider policy.

* Third audit pass: core resource limits now protect Flutter translation rendering against excessive reference expansion/nesting; invalid dates and non-finite digit options no longer throw from the built-in formatter. Added a widget regression and documented host input/loader limits.

* Second audit pass: preserve mounted UI for synchronously ready catalogs; use snapshot/future identity instead of build-scoped listener callbacks for scope revisions; preserve host direction without a Fluent scope.
* Catalog readiness uses the core's `FutureOr<void>` hook. Added lifecycle tests for eager/cached catalogs, replacement during loading, unmount, synchronous refresh and fallback reconfiguration.

* Fix lazy translation reactivity in `trProvider`, nested composite loading, nested `ProviderScope` isolation, and same-frame retry notifications in `RiverpodFluentScope`.
* `trProvider` and `localeReadyProvider` now auto-dispose; custom providers watching them must also be auto-dispose providers. Catalogs retain loaded data and in-flight requests.
* Validate fallback provider configuration; reject malformed locale extensions and separators through the core parser.
* Correct monorepo installation, loader failure handling, cache/retry and Flutter framework integration guidance.

* `localeReadyProvider` loads a `LazyCatalog` for the active locale and its fallback chain.
* `RiverpodFluentScope.loading` shows a placeholder while a lazy locale loads. `FluentScope.revision` makes dependents rebuild when messages become available.
* Widget and Riverpod tests added in `test/`.

* `RiverpodFluentScope`: publishes Riverpod's locale and `UiMessages` into the widget tree, making Riverpod the single source of truth.
* `setLocale` and the initial locale now validate BCP-47 tags and store the canonical form; invalid tags throw `ArgumentError`.
* `currentLocaleProvider` starts from `defaultLocaleProvider` instead of a hard-coded `'en'`.
* `messageCatalogProvider` has no default catalog and throws `UnimplementedError` until overridden (previously it silently used an empty catalog).
* `DirectionalityHandler` documentation now shows the `MaterialApp.builder` placement.
* `flutter_riverpod` is a required dependency (it was documented as optional).
* The engine behind this package accepts nested placeables and attaches translator comments to messages and terms (`FtlMessage.comment`), so `# ...` comment blocks used as translator notes no longer show up as separate entries when the catalogs are inspected.
* README: documented the `localeReadyProvider` error behaviour (a failed `LazyCatalog` load is not retried; invalidate the provider to retry), dotted attribute keys, and which extensions require a `FluentScope` ancestor.

## 0.1.0

* Initial release of `rustok_flutter_i18n`.
* `FluentScope` InheritedWidget for injecting internationalization into the widget tree.
* `DirectionalityHandler` for automatic RTL/LTR text direction layout wrapping.
* `BuildContext.t()` and `BuildContext.tAttr()` convenience extensions.
* Safe `BuildContext.maybeT()` for unmounted preview environments and tests.
* Riverpod bindings: `currentLocaleProvider`, `uiMessagesProvider`, `isRtlLocaleProvider`, and reactive `trProvider`.
