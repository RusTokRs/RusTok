class RouteSelection {
  const RouteSelection({required this.path, this.query = const {}});

  final String path;
  final Map<String, String> query;

  String toLocation() {
    if (query.isEmpty) {
      return path;
    }
    final pairs = query.entries
        .map((entry) => '${Uri.encodeQueryComponent(entry.key)}=${Uri.encodeQueryComponent(entry.value)}')
        .join('&');
    return '$path?$pairs';
  }
}
