import pathlib
import unittest

from rustok_mobile.tooling.scripts.verify_cldr_plurals import (
    DART_TABLE,
    _bodies,
    load_rule,
    operands,
    structural_problems,
)

# CLDR 48 publishes no cardinal category other than `other` for these tags, so
# `resolveCldrPluralCategory` must answer `other` for every value of them.
ONLY_OTHER_TAGS = (
    'bm', 'bo', 'dz', 'hnj', 'id', 'ig', 'ii', 'ja', 'jbo', 'jv', 'jw', 'kde',
    'kea', 'km', 'ko', 'lkt', 'lo', 'ms', 'my', 'nqo', 'osa', 'sah', 'ses',
    'sg', 'su', 'th', 'to', 'tpi', 'und', 'vi', 'wo', 'yo', 'yue', 'zh',
)

# Tags whose CLDR rule is stated by the table's own doc comment samples;
# `pt-pt` is what `pt-PT` normalizes to before the switch.
CATEGORY_SAMPLES = (
    ('ru', 1, 'one'), ('ru', 2, 'few'), ('ru', 5, 'many'), ('ru', 1.5, 'other'),
    ('pl', 1, 'one'), ('pl', 3, 'few'), ('pl', 12, 'many'), ('pl', 1.5, 'other'),
    ('he', 1, 'one'), ('he', 2, 'two'), ('he', 3, 'other'),
    ('cy', 0, 'zero'), ('cy', 6, 'many'), ('da', 0.5, 'one'),
    ('hi', 0, 'one'), ('fr', 1000000, 'many'), ('pt-pt', 1, 'one'),
    ('pt', 0, 'one'), ('pt', 1000000, 'many'), ('sl', 2, 'two'),
    ('sl', 5, 'other'), ('tzm', 11, 'one'), ('lt', 1.2, 'many'),
    ('lv', 0.1, 'one'), ('ro', 101, 'few'), ('ar', 0, 'zero'),
    ('ar', 2, 'two'), ('ar', 3.5, 'other'), ('unknown', 1, 'other'),
)


class CldrPluralsTableTests(unittest.TestCase):
    def setUp(self):
        self.lines = DART_TABLE.read_text(encoding='utf-8').split('\n')
        self.rule, self.groups, self.problems = load_rule()

    def test_no_case_body_can_complete_normally(self):
        # A body that can complete normally is rejected by the Dart compiler
        # ("The body might complete normally...") and would otherwise fall
        # through into the next rule.
        self.assertEqual(self.problems, [])
        self.assertEqual(structural_problems(self.groups), [])

    def test_the_switch_ends_with_a_default_returning_other(self):
        last = self.groups[-1]
        self.assertTrue(last['default'])
        self.assertEqual([line.strip() for _, line in last['body']], ["return 'other';"])

    def test_every_published_tag_has_a_case(self):
        tags = [group['tag'] for group in self.groups if group['tag']]
        self.assertEqual(len(tags), 223)
        self.assertEqual(len(set(tags)), len(tags))
        self.assertIn('pt-pt', tags)

    def test_only_other_tags_return_other_without_falling_through(self):
        bodies = {}
        pending = []
        for group in self.groups:
            if group['default']:
                continue
            tags = pending + [group['tag']]
            if not group['body']:
                pending = tags
                continue
            pending = []
            for tag in tags:
                bodies[tag] = [line.strip() for _, line in group['body']]
        for tag in ONLY_OTHER_TAGS:
            self.assertIn(tag, bodies, f'{tag} has no case body')
            self.assertEqual(
                bodies[tag],
                ["return 'other';"],
                f'{tag} must resolve every value to `other`',
            )

    def test_documented_samples_resolve_as_documented(self):
        for language, value, expected in CATEGORY_SAMPLES:
            with self.subTest(language=language, value=value):
                self.assertEqual(self.rule(language, **operands(value)), expected)

    def test_operands_match_the_dart_helpers(self):
        # `_visibleFractionDigits` / `_fractionDigits` treat integral values and
        # exponential notation as having no visible fraction digits.
        self.assertEqual(operands(1)['v'], 0)
        self.assertEqual(operands(1.0)['v'], 0)
        self.assertEqual(operands(1.25)['v'], 2)
        self.assertEqual(operands(1.25)['f'], 25)
        self.assertEqual(operands(1e21)['v'], 0)
        self.assertEqual(operands(1e21)['i'], 10 ** 21)

    def test_the_table_is_shaped_like_the_generator_output(self):
        # The verifier reads the table by shape; keep it a single `switch`.
        text = '\n'.join(self.lines)
        self.assertEqual(text.count('switch (lang) {'), 1)
        self.assertNotIn('case ', text[text.index('switch (lang) {'):][:0] + '')
        self.assertTrue(all(group['line'] for group in self.groups))
        self.assertTrue(pathlib.Path(DART_TABLE).is_file())


if __name__ == '__main__':
    unittest.main()
