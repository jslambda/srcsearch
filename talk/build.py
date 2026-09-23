#!/usr/bin/env python3
"""Bundle the presentation for offline viewing (Python standard library only)."""

import argparse
import html
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile

HERE = Path(__file__).resolve().parent
REVEAL = "https://cdn.jsdelivr.net/npm/reveal.js@6.0.1/"
MATHJAX = "https://cdn.jsdelivr.net/npm/mathjax@4.0.0/tex-mml-chtml.js"
PACKAGES = {
    "reveal": "https://registry.npmjs.org/reveal.js/-/reveal.js-6.0.1.tgz",
    "mathjax": "https://registry.npmjs.org/mathjax/-/mathjax-4.0.0.tgz",
    "font": "https://registry.npmjs.org/@mathjax/mathjax-newcm-font/-/mathjax-newcm-font-4.0.0.tgz",
}


def unpack(archive, destination):
    """Copy regular package files only; never follow archive links or traversal."""
    with tarfile.open(archive) as package:
        for member in package.getmembers():
            parts = Path(member.name).parts
            if not parts or parts[0] != "package" or ".." in parts:
                raise ValueError(f"Unsafe package path: {member.name}")
            if not member.isfile():
                continue
            target = destination.joinpath(*parts[1:])
            target.parent.mkdir(parents=True, exist_ok=True)
            with package.extractfile(member) as source, target.open("wb") as output:
                shutil.copyfileobj(source, output)


def render(template, markdown):
    pattern = r'<section\s+data-markdown="presentation.md"([^>]*)></section>'
    # A textarea preserves Markdown literally, including HTML and code samples.
    result, count = re.subn(
        pattern,
        lambda match: '<section data-markdown' + match[1]
        + '><textarea data-template>' + html.escape(markdown)
        + '</textarea></section>',
        template,
    )
    if count != 1 or REVEAL not in result or MATHJAX not in result:
        raise ValueError("Presentation template changed; update the offline builder.")
    result = result.replace(REVEAL, "assets/reveal/")
    # SVG avoids file:// web-font restrictions. Dynamic glyph data stays local.
    result = result.replace(
        f'mathjax: "{MATHJAX}"',
        'mathjax: "assets/mathjax/tex-mml-svg.js",\n'
        '          loader: { paths: { "mathjax-newcm": "assets/font" } },\n'
        '          output: { fontPath: "assets/font" },\n'
        '          options: { enableMenu: false }',
    )
    return result.replace(
        "Start talk/runserver.sh and open the shown http:// address.",
        "Rebuild with talk/build.sh and keep the output folder together.",
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=HERE / "dist",
                        help="output directory (default: talk/dist)")
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists():
        parser.error(f"Output already exists: {output}; choose a new --output directory")
    page = render((HERE / "index.html").read_text(encoding="utf-8"),
                  (HERE / "presentation.md").read_text(encoding="utf-8"))
    cache = HERE / ".build-cache"
    cache.mkdir(exist_ok=True)
    output.parent.mkdir(parents=True, exist_ok=True)
    # Publish only a complete build, leaving no partial output after a failure.
    with tempfile.TemporaryDirectory(dir=output.parent) as temporary:
        stage = Path(temporary) / "dist"
        stage.mkdir()
        for name, url in PACKAGES.items():
            archive = cache / url.rsplit("/", 1)[1]
            if not archive.exists():
                print(f"Downloading {name}…", flush=True)
                partial = Path(temporary) / archive.name
                subprocess.run(["curl", "--fail", "--location", "--retry", "2",
                                "--connect-timeout", "15", "--max-time", "180",
                                url, "--output", str(partial)], check=True)
                unpack(partial, stage / "assets" / name)
                shutil.copyfile(partial, archive)
            else:
                unpack(archive, stage / "assets" / name)
        # Keep the deck's local benchmark link useful when moving the bundle.
        benchmark = "benchmarks/ripgrep-codex.md"
        (stage / "benchmarks").mkdir()
        shutil.copyfile(HERE.parent / benchmark, stage / benchmark)
        page = page.replace("../" + benchmark, benchmark)
        (stage / "index.html").write_text(page, encoding="utf-8")
        stage.rename(output)
    print(f"Open {output / 'index.html'} in your browser (no server needed).")


if __name__ == "__main__":
    main()
