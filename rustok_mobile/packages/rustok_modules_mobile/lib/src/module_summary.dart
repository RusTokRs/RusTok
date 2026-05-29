class ModuleSummary {
  const ModuleSummary({
    required this.slug,
    required this.name,
    required this.description,
    required this.version,
    required this.kind,
    required this.enabled,
    required this.ownership,
    required this.trustLevel,
    required this.recommendedAdminSurfaces,
    required this.showcaseAdminSurfaces,
  });

  final String slug;
  final String name;
  final String description;
  final String version;
  final String kind;
  final bool enabled;
  final String ownership;
  final String trustLevel;
  final List<String> recommendedAdminSurfaces;
  final List<String> showcaseAdminSurfaces;

  factory ModuleSummary.fromJson(Map<String, dynamic> json) {
    return ModuleSummary(
      slug: _readString(json, 'moduleSlug'),
      name: _readString(json, 'name'),
      description: _readString(json, 'description'),
      version: _readString(json, 'version'),
      kind: _readString(json, 'kind'),
      enabled: json['enabled'] == true,
      ownership: _readString(json, 'ownership'),
      trustLevel: _readString(json, 'trustLevel'),
      recommendedAdminSurfaces: _readStringList(json, 'recommendedAdminSurfaces'),
      showcaseAdminSurfaces: _readStringList(json, 'showcaseAdminSurfaces'),
    );
  }
}

String _readString(Map<String, dynamic> json, String key) {
  final value = json[key];
  return value is String ? value : '';
}

List<String> _readStringList(Map<String, dynamic> json, String key) {
  final value = json[key];
  if (value is! List) {
    return const <String>[];
  }
  return List.unmodifiable(value.whereType<String>());
}
