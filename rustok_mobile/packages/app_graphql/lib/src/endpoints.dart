class GraphQlEndpoints {
  const GraphQlEndpoints({
    this.httpPath = '/api/graphql',
    this.wsPath = '/api/graphql/ws',
  });

  final String httpPath;
  final String wsPath;

  Uri httpUri(Uri baseUri) => _resolve(baseUri, httpPath);

  Uri wsUri(Uri baseUri) {
    final resolved = _resolve(baseUri, wsPath);
    final wsScheme = resolved.scheme == 'https' ? 'wss' : 'ws';
    return resolved.replace(scheme: wsScheme);
  }

  Uri _resolve(Uri baseUri, String path) {
    final normalizedPath = path.startsWith('/') ? path.substring(1) : path;
    final root = baseUri.path.endsWith('/') ? baseUri.path : '${baseUri.path}/';
    return baseUri.replace(path: '$root$normalizedPath').normalizePath();
  }
}
