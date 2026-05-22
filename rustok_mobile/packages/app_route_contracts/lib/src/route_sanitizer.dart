class RouteSanitizer {
  const RouteSanitizer(this.allowedQueryKeys);

  final Set<String> allowedQueryKeys;

  Map<String, String> sanitize(Map<String, String> source) {
    return Map.unmodifiable({
      for (final entry in source.entries)
        if (allowedQueryKeys.contains(entry.key) && entry.value.isNotEmpty)
          entry.key: entry.value,
    });
  }
}
