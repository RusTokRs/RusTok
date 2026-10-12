import pathlib
import unittest


class RustokI18nPackageTests(unittest.TestCase):
    def setUp(self):
        self.repo_root = pathlib.Path(__file__).resolve().parents[3]
        self.i18n_pkg = self.repo_root / "rustok_mobile/packages/rustok_i18n"
        self.flutter_i18n_pkg = (
            self.repo_root / "rustok_mobile/packages/rustok_flutter_i18n"
        )

    def test_package_directories_and_pubspecs_exist(self):
        self.assertTrue((self.i18n_pkg / "pubspec.yaml").is_file())
        self.assertTrue((self.flutter_i18n_pkg / "pubspec.yaml").is_file())
        self.assertTrue((self.i18n_pkg / "docs/implementation-plan.md").is_file())

    def test_core_library_structure_and_modules(self):
        lib = self.i18n_pkg / "lib"
        self.assertTrue((lib / "rustok_i18n.dart").is_file())

        expected_modules = [
            "src/ast/ast_nodes.dart",
            "src/parser/ftl_parser.dart",
            "src/resolver/cldr_plurals.dart",
            "src/resolver/fluent_resolver.dart",
            "src/locale/locale_tag.dart",
            "src/locale/fallback_chain.dart",
            "src/locale/text_direction.dart",
            "src/accept_language/accept_language_parser.dart",
            "src/catalog/message_catalog.dart",
            "src/messages/resolved_message.dart",
            "src/messages/ui_messages.dart",
        ]

        for mod in expected_modules:
            self.assertTrue(
                (lib / mod).is_file(),
                f"Missing expected core i18n module: {mod}",
            )

    def test_flutter_library_structure_and_modules(self):
        lib = self.flutter_i18n_pkg / "lib"
        self.assertTrue((lib / "rustok_flutter_i18n.dart").is_file())

        expected_modules = [
            "src/widgets/fluent_scope.dart",
            "src/extensions/fluent_extensions.dart",
            "src/riverpod/fluent_providers.dart",
        ]

        for mod in expected_modules:
            self.assertTrue(
                (lib / mod).is_file(),
                f"Missing expected flutter i18n module: {mod}",
            )

    def test_cli_tool_exists_with_check_and_typegen(self):
        cli = self.i18n_pkg / "bin/i18n_cli.dart"
        self.assertTrue(cli.is_file())
        content = cli.read_text(encoding="utf-8")
        self.assertIn("check", content)
        self.assertIn("typegen", content)
        self.assertIn("_runCheck", content)
        self.assertIn("_runTypegen", content)

    def test_dos_guard_and_bounded_input_constants(self):
        locale_tag = (self.i18n_pkg / "lib/src/locale/locale_tag.dart").read_text(
            encoding="utf-8"
        )
        self.assertIn("maxLocaleTagLength = 64", locale_tag)

        ui_messages = (self.i18n_pkg / "lib/src/messages/ui_messages.dart").read_text(
            encoding="utf-8"
        )
        self.assertIn("maxMessageKeyLength = 128", ui_messages)

        accept_lang = (
            self.i18n_pkg / "lib/src/accept_language/accept_language_parser.dart"
        ).read_text(encoding="utf-8")
        self.assertIn("maxAcceptLanguageLength = 4096", accept_lang)
        self.assertIn("maxAcceptLanguageRanges = 64", accept_lang)

    def test_ast_covers_all_fluent_grammar_constructs(self):
        ast = (self.i18n_pkg / "lib/src/ast/ast_nodes.dart").read_text(encoding="utf-8")
        required_nodes = [
            "class FtlResource",
            "class FtlMessage",
            "class FtlTerm",
            "class FtlAttribute",
            "class FtlComment",
            "class FtlPattern",
            "class FtlTextElement",
            "class FtlPlaceable",
            "class FtlStringLiteral",
            "class FtlNumberLiteral",
            "class FtlVariableReference",
            "class FtlTermReference",
            "class FtlMessageReference",
            "class FtlSelectExpression",
            "class FtlSelectVariant",
        ]
        for node in required_nodes:
            self.assertIn(node, ast, f"AST missing required node: {node}")

    def test_cldr_plurals_implements_all_slavic_and_world_categories(self):
        cldr = (self.i18n_pkg / "lib/src/resolver/cldr_plurals.dart").read_text(
            encoding="utf-8"
        )
        self.assertIn("resolveCldrPluralCategory", cldr)
        self.assertIn("'ru'", cldr)
        self.assertIn("'uk'", cldr)
        self.assertIn("'fr'", cldr)
        self.assertIn("'ar'", cldr)
        self.assertIn("'one'", cldr)
        self.assertIn("'few'", cldr)
        self.assertIn("'many'", cldr)
        self.assertIn("'other'", cldr)

    def test_resolver_implements_bidi_isolation_and_cycle_guard(self):
        resolver = (self.i18n_pkg / "lib/src/resolver/fluent_resolver.dart").read_text(
            encoding="utf-8"
        )
        self.assertIn("bidiFsi = '\\u2068'", resolver)
        self.assertIn("bidiPdi = '\\u2069'", resolver)
        self.assertIn("useIsolating", resolver)
        self.assertIn("_maxDepth = 100", resolver)
        self.assertIn("formatTerm", resolver)

    def test_catalog_architecture_supports_multiple_backends(self):
        catalog = (self.i18n_pkg / "lib/src/catalog/message_catalog.dart").read_text(
            encoding="utf-8"
        )
        self.assertIn("abstract class MessageCatalog", catalog)
        self.assertIn("class FluentCatalog extends MessageCatalog", catalog)
        self.assertIn("class StaticCatalog extends MessageCatalog", catalog)
        self.assertIn("class JsonCatalog extends MessageCatalog", catalog)
        self.assertIn("class CompositeCatalog extends MessageCatalog", catalog)

    def test_provenance_diagnostics_implemented(self):
        provenance = (
            self.i18n_pkg / "lib/src/messages/resolved_message.dart"
        ).read_text(encoding="utf-8")
        self.assertIn("enum ResolutionProvenance", provenance)
        self.assertIn("exactMatch", provenance)
        self.assertIn("parentLanguage", provenance)
        self.assertIn("defaultLocale", provenance)
        self.assertIn("missingKeyFallback", provenance)
        self.assertIn("class ResolvedMessage", provenance)

    def test_ui_messages_supports_dotted_and_kebab_aliases(self):
        messages = (self.i18n_pkg / "lib/src/messages/ui_messages.dart").read_text(
            encoding="utf-8"
        )
        self.assertIn("_normalizeKebabKey", messages)
        self.assertIn("_normalizeDottedKey", messages)
        self.assertIn("resolve", messages)
        self.assertIn("t(", messages)
        self.assertIn("tAttr(", messages)

    def test_flutter_scope_manages_directionality_and_locale(self):
        scope = (
            self.flutter_i18n_pkg / "lib/src/widgets/fluent_scope.dart"
        ).read_text(encoding="utf-8")
        self.assertIn("class FluentScope extends InheritedWidget", scope)
        self.assertIn("class DirectionalityHandler extends StatelessWidget", scope)
        self.assertIn("flutterTextDirection", scope)
        self.assertIn("isRtl", scope)

    def test_extensions_and_riverpod_wiring(self):
        extensions = (
            self.flutter_i18n_pkg / "lib/src/extensions/fluent_extensions.dart"
        ).read_text(encoding="utf-8")
        self.assertIn("extension FluentBuildContext on BuildContext", extensions)
        self.assertIn("String t(", extensions)
        self.assertIn("String tAttr(", extensions)

        providers = (
            self.flutter_i18n_pkg / "lib/src/riverpod/fluent_providers.dart"
        ).read_text(encoding="utf-8")
        self.assertIn("class LocaleNotifier extends StateNotifier<String>", providers)
        self.assertIn("final currentLocaleProvider", providers)
        self.assertIn("final uiMessagesProvider", providers)
        self.assertIn("final trProvider", providers)
        self.assertIn("final isRtlLocaleProvider", providers)

    def test_legacy_android_aliases_and_dos_bounds(self):
        locale_tag = (self.i18n_pkg / "lib/src/locale/locale_tag.dart").read_text(
            encoding="utf-8"
        )
        self.assertIn("legacyAliases", locale_tag)
        self.assertIn("'iw': 'he'", locale_tag)
        self.assertIn("'in': 'id'", locale_tag)
        self.assertIn("'ji': 'yi'", locale_tag)

    def test_text_direction_avoids_flutter_name_collision(self):
        text_dir = (
            self.i18n_pkg / "lib/src/locale/text_direction.dart"
        ).read_text(encoding="utf-8")
        self.assertIn("enum LocaleTextDirection", text_dir)
        self.assertIn("typedef TextDirection = LocaleTextDirection", text_dir)

    def test_parser_handles_crlf_multiline_negative_numbers_and_escapes(self):
        parser_code = (
            self.i18n_pkg / "lib/src/parser/ftl_parser.dart"
        ).read_text(encoding="utf-8")
        self.assertIn("replaceAll('\\r\\n', '\\n')", parser_code)
        self.assertIn("_dedentPattern", parser_code)
        self.assertIn("_parsePatternOrMultiline", parser_code)
        self.assertIn("char == '-' && _isDigit(_peek(1))", parser_code)
        self.assertIn("case 'n':", parser_code)

    def test_resolver_strips_bidi_and_has_cycle_guard(self):
        resolver_code = (
            self.i18n_pkg / "lib/src/resolver/fluent_resolver.dart"
        ).read_text(encoding="utf-8")
        self.assertIn("cleanSelector", resolver_code)
        self.assertIn("variantNum == numericValue", resolver_code)
        self.assertIn("_activeEntries", resolver_code)
        self.assertIn("_activeEntries.contains(entryKey)", resolver_code)

    def test_ast_memoizes_resource_lookup_maps(self):
        ast_code = (self.i18n_pkg / "lib/src/ast/ast_nodes.dart").read_text(
            encoding="utf-8"
        )
        self.assertIn("late final Map<String, FtlMessage> messages;", ast_code)
        self.assertIn("late final Map<String, FtlTerm> terms;", ast_code)

    def test_catalogs_support_attribute_lookups(self):
        catalog_code = (
            self.i18n_pkg / "lib/src/catalog/message_catalog.dart"
        ).read_text(encoding="utf-8")
        self.assertIn("table['$id.$attribute'] ?? table[id]", catalog_code)
        self.assertIn("Map<dynamic, dynamic>", catalog_code)

    def test_accept_language_guarantees_stable_sort(self):
        accept_code = (
            self.i18n_pkg / "lib/src/accept_language/accept_language_parser.dart"
        ).read_text(encoding="utf-8")
        self.assertIn("final int order;", accept_code)
        self.assertIn("a.order.compareTo(b.order)", accept_code)

    def test_flutter_scope_has_state_error_and_safe_helpers(self):
        scope_code = (
            self.flutter_i18n_pkg / "lib/src/widgets/fluent_scope.dart"
        ).read_text(encoding="utf-8")
        self.assertIn("throw StateError(", scope_code)

        ext_code = (
            self.flutter_i18n_pkg / "lib/src/extensions/fluent_extensions.dart"
        ).read_text(encoding="utf-8")
        self.assertIn("String maybeT(", ext_code)

    def test_ui_messages_supports_dotted_attribute_and_snake_case(self):
        messages_code = (
            self.i18n_pkg / "lib/src/messages/ui_messages.dart"
        ).read_text(encoding="utf-8")
        self.assertIn("lastDot", messages_code)
        self.assertIn("msgPart", messages_code)
        self.assertIn("attrPart", messages_code)
        self.assertIn(".replaceAll('_', '-')", messages_code)
        self.assertIn("class UiLocaleTranslator", messages_code)
        self.assertIn("forLocale(", messages_code)
        self.assertIn("tryResolve(", messages_code)

    def test_admin_locale_normalization_parity(self):
        locale_code = (
            self.i18n_pkg / "lib/src/locale/locale_tag.dart"
        ).read_text(encoding="utf-8")
        self.assertIn("String normalizeAdminLocale(String? locale)", locale_code)

    def test_typegen_escapes_reserved_keywords(self):
        cli_code = (self.i18n_pkg / "bin/i18n_cli.dart").read_text(encoding="utf-8")
        self.assertIn("_dartReservedWords", cli_code)
        self.assertIn("_toDartMethodName", cli_code)
        self.assertIn("_toDartParamName", cli_code)
        self.assertIn("recursive", cli_code)

    def test_accept_language_locales_and_preferences_parity(self):
        accept_code = (
            self.i18n_pkg / "lib/src/accept_language/accept_language_parser.dart"
        ).read_text(encoding="utf-8")
        self.assertIn("List<String> acceptLanguageLocales(", accept_code)
        self.assertIn("List<String> acceptLanguageCatalogLocales(", accept_code)
        self.assertIn("String? preferredLocaleFromAcceptLanguage(", accept_code)
        self.assertIn("String? preferredCatalogLocaleFromAcceptLanguage(", accept_code)

    def test_ui_locale_translator_reuses_candidates_without_realloc(self):
        messages_code = (
            self.i18n_pkg / "lib/src/messages/ui_messages.dart"
        ).read_text(encoding="utf-8")
        self.assertIn("_resolveWithCandidates", messages_code)
        self.assertIn("candidates: candidates", messages_code)
        self.assertIn("String normalizeKebabKey(String input)", messages_code)

    def test_riverpod_provides_tr_and_rtl_providers(self):
        riverpod_code = (
            self.flutter_i18n_pkg / "lib/src/riverpod/fluent_providers.dart"
        ).read_text(encoding="utf-8")
        self.assertIn("final trProvider =", riverpod_code)
        self.assertIn("final isRtlLocaleProvider =", riverpod_code)
        self.assertIn("final defaultLocaleProvider =", riverpod_code)


if __name__ == "__main__":
    unittest.main()


