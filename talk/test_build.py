import html
import io
from pathlib import Path
import tarfile
import tempfile
import unittest

from build import HERE, render, unpack


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
