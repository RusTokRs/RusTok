"""Generator path guards; no Dart SDK or network required."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / 'scripts/gen_likely_subtags.py'


class LikelySubtagsGeneratorTests(unittest.TestCase):
    def test_rejects_colliding_output_paths_before_writing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / 'text_direction_data.dart'
            output.write_text('preserve', encoding='utf-8')
            result = subprocess.run([
                sys.executable, str(SCRIPT), '--input', str(root / 'missing.json'),
                '--scripts', str(root / 'missing-scripts.json'), '--output', str(output),
            ], capture_output=True, text=True)
            self.assertEqual(result.returncode, 2)
            self.assertIn('distinct paths', result.stderr)
            self.assertEqual(output.read_text(encoding='utf-8'), 'preserve')

    def test_rejects_overwriting_either_input(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for input_flag in ['--input', '--scripts']:
                for name in ['input.json', 'text_direction_data.dart']:
                    with self.subTest(input=input_flag, name=name):
                        source = root / name
                        source.write_text('preserve', encoding='utf-8')
                        output = source if name == 'input.json' else root / 'likely_subtags_data.dart'
                        other_flag = '--scripts' if input_flag == '--input' else '--input'
                        result = subprocess.run([
                            sys.executable, str(SCRIPT), input_flag, str(source),
                            other_flag, str(root / 'missing.json'), '--output', str(output),
                        ], capture_output=True, text=True)
                        self.assertEqual(result.returncode, 2)
                        self.assertIn('must not overwrite input', result.stderr)
                        self.assertEqual(source.read_text(encoding='utf-8'), 'preserve')
