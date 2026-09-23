import html
import io
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest

from build import HERE, render, unpack


class BuildCommandTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.talk = self.root / 'talk with spaces'
        self.talk.mkdir()
        shutil.copyfile(HERE / 'build.sh', self.talk / 'build.sh')
        binaries = self.root / 'bin'
        binaries.mkdir()
        for name in ('python3', 'pandoc'):
            command = binaries / name
            command.write_text('#!/bin/sh\npwd\nprintf "%s\\n" "$@"\n')
            command.chmod(0o755)
        self.env = dict(os.environ, PATH=str(binaries) + os.pathsep + os.environ['PATH'])

    def run_build(self, *args):
        return subprocess.run(['sh', str(self.talk / 'build.sh'), *args],
                              cwd=self.root, env=self.env, text=True, capture_output=True)

    def test_reveal_default_explicit_mode_and_legacy_output(self):
        for args in ((), ('revealjs',), ('--output', 'slides'),
                     ('revealjs', '--output', 'slides')):
            with self.subTest(args=args):
                result = self.run_build(*args)
                self.assertEqual(result.returncode, 0, result.stderr)
                lines = result.stdout.splitlines()
                self.assertEqual(Path(lines[0]).resolve(), self.root.resolve())
                self.assertEqual(Path(lines[1]).resolve(), (self.talk / 'build.py').resolve())
                self.assertEqual(lines[2:], ['--output', 'slides'] if '--output' in args else [])

    def test_pandoc_runs_from_talk_and_creates_dist(self):
        result = self.run_build('pandoc')
        self.assertEqual(result.returncode, 0, result.stderr)
        lines = result.stdout.splitlines()
        self.assertEqual(Path(lines[0]).resolve(), self.talk.resolve())
        self.assertEqual(lines[1:], ['presentation.md', '--standalone',
                                    '--math-method=mathjax', '-o', 'dist/pandoc.html'])
        self.assertTrue((self.talk / 'dist').is_dir())

    def test_invalid_modes_and_pandoc_options_fail(self):
        for args in (('unknown',), ('pandoc', '--output', 'elsewhere')):
            self.assertEqual(self.run_build(*args).returncode, 2)

    def test_help_does_not_build(self):
        result = self.run_build('--help')
        self.assertEqual(result.returncode, 0)
        self.assertIn('revealjs', result.stdout)
        self.assertIn('pandoc', result.stdout)
        self.assertFalse((self.talk / 'dist').exists())


class BuildTests(unittest.TestCase):
    def test_markdown_is_embedded_without_changing_its_contents(self):
        markdown = '# Slide\n\n</textarea><script>alert(1)</script> & $x$\n---\n'
        page = render((HERE / 'index.html').read_text(), markdown)
        embedded = page.split('<textarea data-template>', 1)[1].split('</textarea>', 1)[0]
        self.assertEqual(html.unescape(embedded), markdown)
        self.assertNotIn('data-markdown="presentation.md"', page)
        self.assertNotIn('https://cdn.jsdelivr.net/', page)
        self.assertIn('assets/mathjax/tex-mml-svg.js', page)
        self.assertIn('"mathjax-newcm": "assets/font"', page)
        self.assertIn('data-separator-notes="^Note:"', page)

    def test_changed_template_fails_instead_of_building_broken_deck(self):
        with self.assertRaises(ValueError):
            render('<html></html>', '# Slide')

    def test_extracts_files_but_rejects_traversal(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / 'package.tgz'
            with tarfile.open(archive, 'w:gz') as package:
                for name in ('package/dist/library.js', 'package/../../escape'):
                    member = tarfile.TarInfo(name)
                    member.size = 4
                    package.addfile(member, io.BytesIO(b'test'))
            with self.assertRaises(ValueError):
                unpack(archive, root / 'assets')
            self.assertEqual((root / 'assets/dist/library.js').read_bytes(), b'test')
            self.assertFalse((root / 'escape').exists())


if __name__ == '__main__':
    unittest.main()
