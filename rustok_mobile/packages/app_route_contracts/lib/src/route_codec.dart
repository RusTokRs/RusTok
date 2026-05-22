import 'route_sanitizer.dart';
import 'route_selection.dart';

class RouteCodec {
  const RouteCodec(this.sanitizer);

  final RouteSanitizer sanitizer;

  RouteSelection decode(String path, Map<String, String> query) {
    return RouteSelection(path: path, query: sanitizer.sanitize(query));
  }
}
