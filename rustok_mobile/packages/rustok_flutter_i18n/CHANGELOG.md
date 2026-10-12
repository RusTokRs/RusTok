# Changelog

## Unreleased

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
