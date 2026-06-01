import 'result.dart';

class PageBuilderErrorCatalog {
  const PageBuilderErrorCatalog._();

  static const validation = 'validation';
  static const sanitize = 'sanitize';
  static const runtime = 'runtime';
  static const featureDisabled = 'feature-disabled';
  static const featureDisabledCode = 'FEATURE_DISABLED';

  static const semantics = <String>[
    validation,
    sanitize,
    runtime,
    featureDisabled,
  ];
}

class PageBuilderErrorViewModel {
  const PageBuilderErrorViewModel({
    required this.kind,
    required this.message,
    required this.operatorGuidance,
  });

  final String kind;
  final String message;
  final String operatorGuidance;
}

class PageBuilderErrorMapper {
  const PageBuilderErrorMapper();

  PageBuilderErrorViewModel resolve(AppError error) {
    if (error.code == PageBuilderErrorCatalog.featureDisabledCode) {
      return PageBuilderErrorViewModel(
        kind: PageBuilderErrorCatalog.featureDisabled,
        message: error.message,
        operatorGuidance:
            'Builder publish is disabled for this tenant. Keep the page readable, ask Platform to check builder.publish.enabled, and use the tenant rollback/change-set runbook if this was unexpected.',
      );
    }

    final normalized = error.message.toLowerCase();
    if (normalized.contains('sanitize') ||
        normalized.contains('sanitization')) {
      return PageBuilderErrorViewModel(
        kind: PageBuilderErrorCatalog.sanitize,
        message: error.message,
        operatorGuidance:
            'Review the GrapesJS payload for unsafe HTML, scripts, URLs, or attributes before retrying.',
      );
    }

    if (normalized.contains('validation') || normalized.contains('invalid')) {
      return PageBuilderErrorViewModel(
        kind: PageBuilderErrorCatalog.validation,
        message: error.message,
        operatorGuidance:
            'Check required page fields and ensure project data is valid grapesjs_v1 JSON.',
      );
    }

    return PageBuilderErrorViewModel(
      kind: PageBuilderErrorCatalog.runtime,
      message: error.message,
      operatorGuidance:
          'Retry after checking server health, tenant context, and page-builder runtime logs.',
    );
  }
}
