# Changelog

## 0.1.0

* Initial release of `rustok_flutter_i18n`.
* `FluentScope` InheritedWidget for injecting internationalization into the widget tree.
* `DirectionalityHandler` for automatic RTL/LTR text direction layout wrapping.
* `BuildContext.t()` and `BuildContext.tAttr()` convenience extensions.
* Safe `BuildContext.maybeT()` for unmounted preview environments and tests.
* Full Riverpod bindings: `currentLocaleProvider`, `uiMessagesProvider`, `isRtlLocaleProvider`, and reactive `trProvider`.
