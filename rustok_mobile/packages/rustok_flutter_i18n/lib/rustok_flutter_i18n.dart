/// Flutter widgets, BuildContext extensions, and Riverpod integration for rustok_i18n.
library rustok_flutter_i18n;

// Re-export core Dart internationalization types so consumers have all primitives
export 'package:rustok_i18n/rustok_i18n.dart';

export 'src/widgets/fluent_scope.dart';
export 'src/extensions/fluent_extensions.dart';
export 'src/riverpod/fluent_providers.dart';
export 'src/riverpod/riverpod_fluent_scope.dart';
