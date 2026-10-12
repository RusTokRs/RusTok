/// Indicates the provenance (origin) of how a translated message was resolved.
enum ResolutionProvenance {
  /// Matched the exact requested locale tag (e.g. `ru-RU`).
  exactMatch,

  /// Matched a parent language or regional script fallback (e.g. `ru`).
  parentLanguage,

  /// Matched the default configured fallback locale (e.g. `en`).
  defaultLocale,

  /// Key was absent across all candidates; fell back to the key or supplied default string.
  missingKeyFallback;

  bool get isFound => this != ResolutionProvenance.missingKeyFallback;
}

/// A fully resolved message with provenance diagnostics.
class ResolvedMessage {
  const ResolvedMessage({
    required this.value,
    this.resolvedLocale,
    required this.provenance,
    required this.originalKey,
  });

  /// The final formatted string value.
  final String value;

  /// The concrete locale tag that produced the value, or `null` if unresolved.
  final String? resolvedLocale;

  /// The origin provenance of this translation.
  final ResolutionProvenance provenance;

  /// The key that was requested.
  final String originalKey;

  @override
  String toString() =>
      'ResolvedMessage("$value", locale: $resolvedLocale, provenance: $provenance)';
}
