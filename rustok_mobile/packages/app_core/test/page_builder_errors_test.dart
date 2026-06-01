import 'package:app_core/app_core.dart';
import 'package:test/test.dart';

void main() {
  group('PageBuilderErrorMapper', () {
    const mapper = PageBuilderErrorMapper();

    test('maps FEATURE_DISABLED to feature-disabled guidance', () {
      final resolved = mapper.resolve(
        const AppError(
          code: PageBuilderErrorCatalog.featureDisabledCode,
          message: 'Feature disabled: builder.publish.enabled',
        ),
      );

      expect(resolved.kind, PageBuilderErrorCatalog.featureDisabled);
      expect(resolved.operatorGuidance, contains('builder.publish.enabled'));
    });

    test('maps sanitize and validation messages to catalog semantics', () {
      expect(
        mapper
            .resolve(
              const AppError(
                code: 'BAD_REQUEST',
                message: 'Sanitization policy rejected payload',
              ),
            )
            .kind,
        PageBuilderErrorCatalog.sanitize,
      );
      expect(
        mapper
            .resolve(
              const AppError(
                code: 'BAD_REQUEST',
                message: 'Validation error: invalid grapesjs_v1 JSON',
              ),
            )
            .kind,
        PageBuilderErrorCatalog.validation,
      );
    });

    test('falls back to runtime for unknown errors', () {
      final resolved = mapper.resolve(
        const AppError(code: 'HTTP_ERROR', message: 'Transport timeout'),
      );

      expect(resolved.kind, PageBuilderErrorCatalog.runtime);
      expect(resolved.operatorGuidance, contains('runtime logs'));
    });
  });
}
