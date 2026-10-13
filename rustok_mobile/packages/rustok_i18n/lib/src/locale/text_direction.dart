import 'locale_tag.dart';
import 'likely_subtags.dart';

part 'text_direction_data.dart';

/// Text directionality representation.
///
/// Named [LocaleTextDirection] (not `TextDirection`) so that importing this
/// package together with `package:flutter/widgets.dart` never produces an
/// ambiguous `TextDirection` name.
enum LocaleTextDirection {
  ltr,
  rtl;

  bool get isRtl => this == LocaleTextDirection.rtl;
  bool get isLtr => this == LocaleTextDirection.ltr;
}

String _normalizeScript(String script) {
  if (script.isEmpty) return script;
  return script.substring(0, 1).toUpperCase() + script.substring(1).toLowerCase();
}

/// Determines the text direction for a language and optional script.
///
/// An explicit script always decides the direction: `ku-Latn` is LTR and
/// `ku-Arab` is RTL. Otherwise CLDR likely subtags infer the script. Unknown
/// languages/scripts fall back to LTR; RTL script metadata is generated from CLDR.
LocaleTextDirection detectTextDirection({required String language, String? script}) {
  if (script != null && script.isNotEmpty && _normalizeScript(script) != 'Zzzz') {
    return _rtlScripts.contains(_normalizeScript(script))
        ? LocaleTextDirection.rtl
        : LocaleTextDirection.ltr;
  }

  final tag = LocaleTag.tryParse(language);
  final inferred = tag == null ? null : maximizeLocale(tag)?.script;
  return _rtlScripts.contains(inferred)
      ? LocaleTextDirection.rtl : LocaleTextDirection.ltr;
}
