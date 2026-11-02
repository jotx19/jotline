#!/usr/bin/env python3
"""Check that relative links in the repository's Markdown files point to existing files.

Usage: python3 tools/check_docs.py

Scans every tracked-looking *.md file (skipping target/ and .git/), resolves each relative
link target against the linking file, and fails with a list of broken links. External links
(http, https, mailto) and pure in-page anchors are not checked.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SKIP_DIRS = {".git", "target", "node_modules"}
LINK = re.compile(r"(?<!!)\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
FENCE = re.compile(r"^(```|~~~)")


def markdown_files() -> list[Path]:
    return sorted(
        path
        for path in ROOT.rglob("*.md")
        if not any(part in SKIP_DIRS for part in path.relative_to(ROOT).parts)
    )


def links(text: str) -> list[tuple[int, str]]:
    found: list[tuple[int, str]] = []
    in_fence = False
    for number, line in enumerate(text.splitlines(), start=1):
        if FENCE.match(line.strip()):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        line = re.sub(r"`[^`]*`", "", line)
        found.extend((number, match.group(1)) for match in LINK.finditer(line))
    return found


def is_external(target: str) -> bool:
    return target.startswith(("http://", "https://", "mailto:", "#"))


def main() -> int:
    broken: list[str] = []
    checked = 0
    for path in markdown_files():
        for line, target in links(path.read_text(encoding="utf-8")):
            if is_external(target):
                continue
            checked += 1
            file_part = target.split("#", 1)[0]
            resolved = (path.parent / file_part).resolve()
            if not resolved.exists():
                broken.append(f"{path.relative_to(ROOT)}:{line}: {target}")
    if broken:
        print("Broken relative links:")
        print("\n".join(f"  {entry}" for entry in broken))
        return 1
    print(f"OK: {checked} relative links in {len(markdown_files())} Markdown files")
    return 0


if __name__ == "__main__":
    sys.exit(main())
