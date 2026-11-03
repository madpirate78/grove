#!/usr/bin/env python3
"""
Fetch entourage symbols from a curated manifest and drop them into data/entourage/.

Usage:
    python3 scripts/fetch_entourage.py             # fetch all items
    python3 scripts/fetch_entourage.py --dry-run   # print what would be fetched
    python3 scripts/fetch_entourage.py --only garden  # one category

The manifest (scripts/entourage_manifest.toml) is the source of truth for *which*
symbols appear in the library and *why* (licence, attribution, real-world size).
The script itself is a mechanical download + normalise pass — rerun it freely.

For each manifest item, the script:
 1. Downloads the URL (raw SVG or PNG).
 2. For SVGs, rewrites the <svg viewBox="..."> attribute to match the manifest's
    width_mm/height_mm so EntourageLibrary picks up the right real-world size.
 3. Writes to data/entourage/{category}/{filename}.
 4. After all items: regenerates data/entourage/LICENSES.md.

Python 3.11+ (tomllib).
Depends on: standard library only (urllib). No third-party deps.
"""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
import urllib.error
import urllib.request
from collections import defaultdict
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
MANIFEST_PATH = REPO_ROOT / "scripts" / "entourage_manifest.toml"
DATA_DIR = REPO_ROOT / "data" / "entourage"
LICENCES_PATH = DATA_DIR / "LICENSES.md"

VALID_CATEGORIES = {"people", "vehicles", "furniture", "garden"}
VALID_LICENCES = {"cc0", "cc-by", "cc-by-sa", "public-domain", "lgpl"}

USER_AGENT = "grove-entourage-fetcher/1.0 (+https://github.com/madpirate78/grove)"


def load_manifest() -> list[dict]:
    with MANIFEST_PATH.open("rb") as f:
        data = tomllib.load(f)
    items = data.get("items", [])
    # Filter out the placeholder entry (empty name/url)
    return [it for it in items if it.get("name") and it.get("url")]


def validate_item(item: dict) -> list[str]:
    errs: list[str] = []
    required = ("name", "category", "filename", "source", "url", "licence", "width_mm", "height_mm")
    for field in required:
        if not item.get(field) and item.get(field) != 0:
            errs.append(f"missing field: {field}")
    if item.get("category") not in VALID_CATEGORIES:
        errs.append(f"invalid category: {item.get('category')}")
    if item.get("licence") not in VALID_LICENCES:
        errs.append(f"invalid licence: {item.get('licence')}")
    if item.get("licence") in {"cc-by", "cc-by-sa", "lgpl"} and not item.get("author"):
        errs.append(f"licence {item.get('licence')} requires author")
    if item.get("width_mm", 0) <= 0 or item.get("height_mm", 0) <= 0:
        errs.append("width_mm/height_mm must be positive")
    return errs


def download(url: str) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(req, timeout=30) as resp:
        return resp.read()


VIEWBOX_RE = re.compile(rb'viewBox\s*=\s*"[^"]*"')
SVG_OPEN_RE = re.compile(rb"<svg\b([^>]*)>", re.IGNORECASE)
WIDTH_ATTR_RE = re.compile(rb'\s(width|height)\s*=\s*"[^"]*"', re.IGNORECASE)


def normalise_svg(data: bytes, width_mm: float, height_mm: float) -> bytes:
    """
    Rewrite the <svg> tag so viewBox = "0 0 width_mm height_mm" and any width=/height=
    attributes are stripped (the loader derives real-world size from viewBox).

    Preserves the rest of the SVG content untouched.
    """
    m = SVG_OPEN_RE.search(data)
    if not m:
        # Not an SVG we can rewrite — return as-is and let the loader handle it.
        return data
    attrs = m.group(1)
    # Strip existing width / height attrs (the loader uses viewBox for mm sizing).
    attrs = WIDTH_ATTR_RE.sub(b"", attrs)
    # Replace or insert viewBox.
    new_viewbox = f'viewBox="0 0 {width_mm:g} {height_mm:g}"'.encode()
    if VIEWBOX_RE.search(attrs):
        attrs = VIEWBOX_RE.sub(new_viewbox, attrs)
    else:
        attrs = b" " + new_viewbox + attrs
    new_open = b"<svg" + attrs + b">"
    return data[: m.start()] + new_open + data[m.end() :]


def write_item(item: dict, dry_run: bool) -> Path:
    dest_dir = DATA_DIR / item["category"]
    dest = dest_dir / item["filename"]
    url = item["url"]
    if dry_run:
        print(f"  would fetch {url} -> {dest.relative_to(REPO_ROOT)}")
        return dest

    dest_dir.mkdir(parents=True, exist_ok=True)

    raw = download(url)
    if item["filename"].lower().endswith(".svg"):
        raw = normalise_svg(raw, float(item["width_mm"]), float(item["height_mm"]))
    dest.write_bytes(raw)
    size_kb = len(raw) / 1024
    print(f"  fetched {item['name']} ({size_kb:.1f} KB) -> {dest.relative_to(REPO_ROOT)}")
    return dest


def write_licences(items: list[dict]) -> None:
    buckets: dict[str, list[dict]] = defaultdict(list)
    for item in items:
        buckets[item["licence"]].append(item)

    lines: list[str] = []
    lines.append("# Entourage Library Licences")
    lines.append("")
    lines.append(
        "This directory contains the entourage symbols used as scale references in Grove plans."
    )
    lines.append(
        "Each entry below records the source, author, and licence for every file shipped in "
        "`data/entourage/`."
    )
    lines.append("")
    lines.append(
        "This file is generated by `scripts/fetch_entourage.py` from `scripts/entourage_manifest.toml`. "
        "Do not edit by hand — update the manifest and re-run the script."
    )
    lines.append("")

    order = ["cc0", "public-domain", "cc-by", "cc-by-sa", "lgpl"]
    headers = {
        "cc0": "## CC0 / Creative Commons Zero (Public Domain Dedication)",
        "public-domain": "## Public Domain",
        "cc-by": "## CC BY — Attribution required",
        "cc-by-sa": "## CC BY-SA — Attribution + ShareAlike",
        "lgpl": "## LGPL — Attribution required, derivative work must preserve licence",
    }
    for licence in order:
        group = buckets.get(licence, [])
        if not group:
            continue
        lines.append(headers[licence])
        lines.append("")
        for item in sorted(group, key=lambda it: (it["category"], it["filename"])):
            author = item.get("author", "").strip()
            source_page = item.get("source_page") or item["url"]
            rel_path = f"{item['category']}/{item['filename']}"
            if licence in {"cc-by", "cc-by-sa", "lgpl"}:
                lines.append(
                    f"- **{rel_path}** — \"{item['name']}\" by {author}, "
                    f"from [{item['source']}]({source_page})."
                )
            else:
                credit = f" (author: {author})" if author else ""
                lines.append(
                    f"- **{rel_path}** — \"{item['name']}\"{credit}, "
                    f"from [{item['source']}]({source_page})."
                )
        lines.append("")

    # Anything on disk without a manifest entry was made for Grove itself
    # (AI-generated composites traced to vector by the extract_entourage_*
    # scripts) and ships under the project's MIT licence.
    attributed = {f"{it['category']}/{it['filename']}" for it in items}
    original = sorted(
        p.relative_to(DATA_DIR).as_posix()
        for p in DATA_DIR.glob("*/*.svg")
        if p.relative_to(DATA_DIR).as_posix() not in attributed
    )
    if original:
        lines.append("## Original Grove artwork — MIT (see the repository `LICENSE`)")
        lines.append("")
        lines.append(
            "Created for Grove: AI-generated top-down composites, traced to pure vector by "
            "`scripts/extract_entourage_from_image.py` / `scripts/extract_entourage_from_svg.py`."
        )
        lines.append("")
        lines.extend(f"- **{rel_path}**" for rel_path in original)
        lines.append("")

    LICENCES_PATH.write_text("\n".join(lines), encoding="utf-8")
    print(
        f"  wrote {LICENCES_PATH.relative_to(REPO_ROOT)} "
        f"({len(items)} attributed, {len(original)} original)"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Fetch entourage symbols from manifest")
    parser.add_argument("--dry-run", action="store_true", help="Print actions without fetching")
    parser.add_argument(
        "--only",
        choices=sorted(VALID_CATEGORIES),
        help="Fetch only one category",
    )
    args = parser.parse_args()

    if not MANIFEST_PATH.exists():
        print(f"error: manifest not found: {MANIFEST_PATH}", file=sys.stderr)
        return 1

    items = load_manifest()
    if not items:
        print("manifest has no items to fetch; regenerating LICENSES.md only.")
        if not args.dry_run:
            write_licences([])
        return 0

    # Validate
    errors: list[str] = []
    for idx, item in enumerate(items):
        for err in validate_item(item):
            errors.append(f"item #{idx} ({item.get('name', '?')}): {err}")
    if errors:
        print("manifest validation failed:", file=sys.stderr)
        for err in errors:
            print(f"  - {err}", file=sys.stderr)
        return 2

    # LICENSES.md must always cover the whole manifest, not just the
    # category being fetched, so keep the full list for write_licences.
    all_items = items
    if args.only:
        items = [it for it in items if it["category"] == args.only]
        if not items:
            print(f"no items in category '{args.only}'", file=sys.stderr)
            return 1

    print(f"fetching {len(items)} item(s)...")
    failures: list[tuple[dict, str]] = []
    for item in items:
        try:
            write_item(item, dry_run=args.dry_run)
        except (urllib.error.URLError, urllib.error.HTTPError, TimeoutError) as e:
            failures.append((item, str(e)))
            print(f"  FAILED {item['name']}: {e}", file=sys.stderr)

    if not args.dry_run:
        write_licences(all_items)

    if failures:
        print(f"\n{len(failures)} item(s) failed. Re-run to retry.", file=sys.stderr)
        return 3
    return 0


if __name__ == "__main__":
    sys.exit(main())
