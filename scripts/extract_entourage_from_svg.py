#!/usr/bin/env python3
"""
Extract individual entourage symbols from a composite SVG (typically an
Inkscape trace-bitmap of an LLM-generated PNG). Unlike the raster
extractor, this preserves TRUE VECTOR output — each extracted symbol is
a small standalone SVG with real path geometry.

Usage:
    python3 scripts/extract_entourage_from_svg.py INPUT.svg

Approach:
  1. Parse the input SVG's single path `d` attribute.
  2. Split into subpaths (each starts with M or m).
  3. Compute each subpath's bounding box in the coordinate system of
     the source path.
  4. Group spatially-close subpaths into "items" using a simple cluster
     by bbox-proximity (helps keep e.g. a car's body, wheels, windows
     together as one symbol rather than 9 separate ones).
  5. Emit one SVG per item, with its own viewBox tight to its content
     and the subpaths translated so the item sits at origin.
  6. Write a JSON staging manifest for labelling, same workflow as the
     raster extractor (scripts/extract_entourage_from_image.py).

After staging, edit scripts/.entourage-staging/manifest.json to set
category + name + real-world mm size per item, then re-run with
--finalise to write the labelled SVGs into data/entourage/.
"""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
STAGING = REPO_ROOT / "scripts" / ".entourage-staging"
DATA_DIR = REPO_ROOT / "data" / "entourage"

# How close two subpath bboxes need to be (relative to their size) to
# be considered part of the same item. 0 means only touching.
CLUSTER_GAP = 1.5  # user-space units — smaller than typical inter-item whitespace


@dataclass
class Subpath:
    source: str              # original path fragment (e.g. "m 100,200 c …")
    bbox: tuple[float, float, float, float]  # (xmin, ymin, xmax, ymax) in source coords
    start_abs: tuple[float, float] = (0.0, 0.0)  # absolute cursor at the start of this subpath's initial M/m


# ---------------------------------------------------------------------------
# Minimal SVG path parser (bbox + literal source retention)
# ---------------------------------------------------------------------------

NUMBER_RE = re.compile(r"[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?")
COMMAND_RE = re.compile(r"[MmLlHhVvCcSsQqTtAaZz]")


def _tokens(d: str):
    """Yield (command_char, [floats]) tuples."""
    i = 0
    while i < len(d):
        c = d[i]
        if c in " \t\n,":
            i += 1
            continue
        if COMMAND_RE.match(c):
            cmd = c
            i += 1
            nums = []
            # Collect numbers that follow until next command character
            while i < len(d):
                c2 = d[i]
                if c2 in " \t\n,":
                    i += 1
                    continue
                if COMMAND_RE.match(c2):
                    break
                m = NUMBER_RE.match(d, i)
                if not m:
                    i += 1
                    continue
                nums.append(float(m.group()))
                i = m.end()
            yield cmd, nums
        else:
            i += 1


def split_subpaths(d: str) -> list[str]:
    """Return the literal path-data substring for each subpath."""
    indices = [m.start() for m in re.finditer(r"[Mm]", d)]
    if not indices:
        return []
    pieces = []
    for k, start in enumerate(indices):
        end = indices[k + 1] if k + 1 < len(indices) else len(d)
        pieces.append(d[start:end].strip())
    return pieces


def parse_all_subpaths(d: str) -> list[Subpath]:
    """Walk the whole path in one pass, tracking the cursor across subpath
    boundaries so each subpath's bbox is in the real coordinate space."""
    pieces = split_subpaths(d)
    subpaths: list[Subpath] = []
    cursor_in = (0.0, 0.0)
    for p in pieces:
        bb, cursor_out = _subpath_bbox_from(p, cursor_in)
        # The absolute start of this subpath is the cursor position after
        # interpreting its leading M/m in the source coord space.
        start_abs = _subpath_start_abs(p, cursor_in)
        subpaths.append(Subpath(source=p, bbox=bb, start_abs=start_abs))
        cursor_in = cursor_out
    return subpaths


def _subpath_start_abs(sub: str, cursor_in: tuple[float, float]) -> tuple[float, float]:
    """Given a subpath and the cursor coming in, return where the initial
    M/m lands in absolute coordinates."""
    m = re.match(r'([Mm])\s*([-0-9.,\s+eE]+?)(?=[A-Za-z]|$)', sub)
    if not m:
        return cursor_in
    cmd = m.group(1)
    nums = [float(x) for x in re.findall(NUMBER_RE, m.group(2))]
    if len(nums) < 2:
        return cursor_in
    x0, y0 = nums[0], nums[1]
    if cmd == "m":
        return (cursor_in[0] + x0, cursor_in[1] + y0)
    return (x0, y0)


def _subpath_bbox_from(sub: str, cursor_in: tuple[float, float]) -> tuple[tuple[float, float, float, float], tuple[float, float]]:
    """Trace one subpath starting from `cursor_in`; return its bbox and
    the cursor position at the end (for the next subpath to start from)."""
    cx, cy = cursor_in
    start_x, start_y = cx, cy
    xmin = math.inf
    ymin = math.inf
    xmax = -math.inf
    ymax = -math.inf

    def record(x, y):
        nonlocal xmin, ymin, xmax, ymax
        if x < xmin: xmin = x
        if y < ymin: ymin = y
        if x > xmax: xmax = x
        if y > ymax: ymax = y

    last_cmd = None
    for cmd, nums in _tokens(sub):
        rel = cmd.islower()
        c = cmd.upper()

        if c == "M":
            # First pair is moveto; subsequent pairs are implicit L.
            if not nums:
                continue
            x, y = nums[0], nums[1]
            if rel:
                x += cx
                y += cy
            cx, cy = x, y
            start_x, start_y = cx, cy
            record(cx, cy)
            i = 2
            while i + 1 < len(nums):
                x, y = nums[i], nums[i + 1]
                if rel:
                    x += cx
                    y += cy
                cx, cy = x, y
                record(cx, cy)
                i += 2
        elif c == "L":
            i = 0
            while i + 1 < len(nums):
                x, y = nums[i], nums[i + 1]
                if rel:
                    x += cx
                    y += cy
                cx, cy = x, y
                record(cx, cy)
                i += 2
        elif c == "H":
            for n in nums:
                x = n + cx if rel else n
                cx = x
                record(cx, cy)
        elif c == "V":
            for n in nums:
                y = n + cy if rel else n
                cy = y
                record(cx, cy)
        elif c == "C":
            i = 0
            while i + 5 < len(nums):
                c1x, c1y, c2x, c2y, ex, ey = nums[i:i+6]
                if rel:
                    c1x += cx; c1y += cy
                    c2x += cx; c2y += cy
                    ex += cx; ey += cy
                record(c1x, c1y)
                record(c2x, c2y)
                cx, cy = ex, ey
                record(cx, cy)
                i += 6
        elif c == "S":
            i = 0
            while i + 3 < len(nums):
                c2x, c2y, ex, ey = nums[i:i+4]
                if rel:
                    c2x += cx; c2y += cy
                    ex += cx; ey += cy
                record(c2x, c2y)
                cx, cy = ex, ey
                record(cx, cy)
                i += 4
        elif c == "Q":
            i = 0
            while i + 3 < len(nums):
                c1x, c1y, ex, ey = nums[i:i+4]
                if rel:
                    c1x += cx; c1y += cy
                    ex += cx; ey += cy
                record(c1x, c1y)
                cx, cy = ex, ey
                record(cx, cy)
                i += 4
        elif c == "T":
            i = 0
            while i + 1 < len(nums):
                ex, ey = nums[i:i+2]
                if rel:
                    ex += cx; ey += cy
                cx, cy = ex, ey
                record(cx, cy)
                i += 2
        elif c == "A":
            # Arc: rx ry xrot large sweep x y
            i = 0
            while i + 6 < len(nums):
                ex, ey = nums[i+5], nums[i+6]
                if rel:
                    ex += cx; ey += cy
                cx, cy = ex, ey
                record(cx, cy)
                i += 7
        elif c == "Z":
            cx, cy = start_x, start_y

        last_cmd = c

    return (xmin, ymin, xmax, ymax), (cx, cy)


# ---------------------------------------------------------------------------
# Clustering subpaths into items
# ---------------------------------------------------------------------------

def _expand(bb, pad):
    return (bb[0] - pad, bb[1] - pad, bb[2] + pad, bb[3] + pad)


def _overlap(a, b):
    return not (a[2] < b[0] or b[2] < a[0] or a[3] < b[1] or b[3] < a[1])


def cluster_subpaths(subpaths: list[Subpath], gap: float) -> list[list[int]]:
    """Union-find: merge subpaths whose bboxes overlap (after expanding by `gap`)."""
    n = len(subpaths)
    parent = list(range(n))

    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    def union(a, b):
        ra, rb = find(a), find(b)
        if ra != rb:
            parent[ra] = rb

    expanded = [_expand(sp.bbox, gap) for sp in subpaths]
    for i in range(n):
        for j in range(i + 1, n):
            if _overlap(expanded[i], expanded[j]):
                union(i, j)

    clusters: dict[int, list[int]] = {}
    for i in range(n):
        r = find(i)
        clusters.setdefault(r, []).append(i)
    return list(clusters.values())


# ---------------------------------------------------------------------------
# Pipeline
# ---------------------------------------------------------------------------

def extract_path_d(svg_text: str) -> tuple[str, float, float]:
    """Return (d, viewbox_width, viewbox_height) from the first <path>."""
    dm = re.search(r'<path[^>]*\sd="([^"]+)"', svg_text)
    if not dm:
        raise RuntimeError("no <path d=...> in the SVG")
    d = dm.group(1)
    vb = re.search(r'viewBox="([^"]+)"', svg_text)
    if vb:
        parts = vb.group(1).split()
        w, h = float(parts[2]), float(parts[3])
    else:
        w = h = 1000.0
    return d, w, h


def translate_subpath(sub: str, start_abs: tuple[float, float], dx: float, dy: float) -> str:
    """Replace the subpath's leading M/m with an absolute M at
    (start_abs.x - dx, start_abs.y - dy). Subsequent pairs after an
    initial `m` are relative and stay untouched (relative l's). Pairs
    after an initial `M` are implicit absolute L's and need shifting.
    """
    m = re.match(r'([Mm])\s*([-0-9.,\s+eE]+?)(?=[A-Za-z]|$)', sub)
    if not m:
        return sub
    cmd = m.group(1)
    nums = [float(x) for x in re.findall(NUMBER_RE, m.group(2))]
    if len(nums) < 2:
        return sub
    new_x = start_abs[0] - dx
    new_y = start_abs[1] - dy
    remaining = nums[2:]
    rebuilt = f"M {new_x:.3f},{new_y:.3f}"
    for i in range(0, len(remaining) - 1, 2):
        rx, ry = remaining[i], remaining[i + 1]
        if cmd == "M":
            rebuilt += f" L {rx - dx:.3f},{ry - dy:.3f}"
        else:
            rebuilt += f" l {rx:.3f},{ry:.3f}"
    tail = sub[m.end():]
    return rebuilt + tail


def stage(svg_path: Path) -> int:
    STAGING.mkdir(parents=True, exist_ok=True)
    for f in STAGING.glob("vec_*.svg"):
        f.unlink()

    svg_text = svg_path.read_text(encoding="utf-8")
    d, vb_w, vb_h = extract_path_d(svg_text)

    subpaths = parse_all_subpaths(d)
    # Drop degenerate subpaths
    subpaths = [s for s in subpaths if s.bbox[2] > s.bbox[0] and s.bbox[3] > s.bbox[1]]
    print(f"parsed {len(subpaths)} non-degenerate subpaths")

    clusters = cluster_subpaths(subpaths, gap=CLUSTER_GAP)
    # Ignore tiny clusters (noise)
    clusters = [c for c in clusters if _cluster_bbox(subpaths, c)[2] - _cluster_bbox(subpaths, c)[0] > 4
                                    and _cluster_bbox(subpaths, c)[3] - _cluster_bbox(subpaths, c)[1] > 4]
    # Sort reading-order
    clusters.sort(key=lambda c: (int(_cluster_bbox(subpaths, c)[1] // 20), _cluster_bbox(subpaths, c)[0]))

    manifest = []
    for idx, cluster in enumerate(clusters):
        xmin, ymin, xmax, ymax = _cluster_bbox(subpaths, cluster)
        w = xmax - xmin
        h = ymax - ymin
        pad = max(w, h) * 0.03
        vb_x, vb_y = xmin - pad, ymin - pad
        vb_width = w + 2 * pad
        vb_height = h + 2 * pad

        # Rebuild a mini-SVG with all subpaths in this cluster, translated
        # so the cluster origin is at (pad, pad).
        path_pieces = []
        for si in cluster:
            moved = translate_subpath(subpaths[si].source, subpaths[si].start_abs, vb_x, vb_y)
            path_pieces.append(moved)
        d_joined = " ".join(path_pieces)

        # Inkscape trace-bitmap produces filled regions (not strokes) — the
        # path's fill IS the ink. We reproduce that faithfully. fill-rule
        # "evenodd" lets interior holes (like a window inside a car roof)
        # cut out of their parent shape correctly.
        svg = (
            f'<?xml version="1.0" encoding="UTF-8"?>\n'
            f'<svg xmlns="http://www.w3.org/2000/svg" '
            f'viewBox="0 0 {vb_width:g} {vb_height:g}">\n'
            f'  <path d="{d_joined}" fill="#2a2a2a" fill-rule="evenodd" stroke="none"/>\n'
            f'</svg>\n'
        )
        out_path = STAGING / f"vec_{idx:02d}.svg"
        out_path.write_text(svg, encoding="utf-8")
        print(f"  #{idx:02d} bbox {xmin:.1f},{ymin:.1f} .. {xmax:.1f},{ymax:.1f}  "
              f"{w:.1f}×{h:.1f}  subpaths={len(cluster)}")

        manifest.append({
            "index": idx,
            "vec_file": out_path.name,
            "source_bbox": [xmin, ymin, xmax, ymax],
            "size_source": [w, h],
            "subpath_count": len(cluster),
            "category": "",
            "name": "",
            "filename": "",
            "width_mm": 0,
            "height_mm": 0,
            "skip": False,
        })

    (STAGING / "manifest.json").write_text(json.dumps(manifest, indent=2))
    print(f"\nstaged {len(clusters)} vector clusters to {STAGING.relative_to(REPO_ROOT)}/")
    return 0


def _cluster_bbox(subpaths: list[Subpath], indices: list[int]) -> tuple[float, float, float, float]:
    xs_min = min(subpaths[i].bbox[0] for i in indices)
    ys_min = min(subpaths[i].bbox[1] for i in indices)
    xs_max = max(subpaths[i].bbox[2] for i in indices)
    ys_max = max(subpaths[i].bbox[3] for i in indices)
    return (xs_min, ys_min, xs_max, ys_max)


def finalise() -> int:
    """Read the labelled manifest and rewrite each vec_XX.svg with the
    real-world mm viewBox into data/entourage/{category}/{filename}."""
    manifest_path = STAGING / "manifest.json"
    if not manifest_path.exists():
        print(f"no manifest at {manifest_path} — run staging first", file=sys.stderr)
        return 1
    entries = json.loads(manifest_path.read_text())
    written = 0
    for e in entries:
        if e.get("skip"):
            continue
        if not e.get("category") or not e.get("filename"):
            continue
        if e["width_mm"] <= 0 or e["height_mm"] <= 0:
            continue
        src_path = STAGING / e["vec_file"]
        if not src_path.exists():
            continue
        svg = src_path.read_text(encoding="utf-8")
        # Replace the viewBox units with mm — preserve the numeric ranges.
        # The staged SVG has viewBox "0 0 vb_w vb_h" in source-unit space;
        # rewriting the outer viewBox to "0 0 width_mm height_mm" and scaling
        # the path is simplest via a wrapping <g transform="scale(…)">.
        old_vb = re.search(r'viewBox="0 0 ([0-9.e+-]+) ([0-9.e+-]+)"', svg)
        if not old_vb:
            continue
        src_w = float(old_vb.group(1))
        src_h = float(old_vb.group(2))
        sx = e["width_mm"] / src_w
        sy = e["height_mm"] / src_h
        new_svg = re.sub(
            r'viewBox="0 0 [0-9.e+-]+ [0-9.e+-]+"',
            f'viewBox="0 0 {e["width_mm"]:g} {e["height_mm"]:g}"',
            svg, count=1,
        )
        # Wrap the <path> in a scaling <g> so its coords map to mm space.
        new_svg = new_svg.replace(
            '<path',
            f'<g transform="scale({sx:g} {sy:g})"><path',
            1,
        ).replace("</svg>", "</g></svg>")
        dest_dir = DATA_DIR / e["category"]
        dest_dir.mkdir(parents=True, exist_ok=True)
        dest = dest_dir / e["filename"]
        dest.write_text(new_svg, encoding="utf-8")
        written += 1
        print(f"  wrote {dest.relative_to(REPO_ROOT)}  ({e['width_mm']}×{e['height_mm']}mm)")
    print(f"\nfinalised {written} vector symbol(s)")
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[1])
    p.add_argument("svg", nargs="?", help="Input composite SVG")
    p.add_argument("--finalise", action="store_true")
    args = p.parse_args()
    if args.finalise:
        return finalise()
    if not args.svg:
        p.error("provide an SVG path, or --finalise")
    path = Path(args.svg).expanduser()
    if not path.exists():
        print(f"not found: {path}", file=sys.stderr)
        return 1
    return stage(path)


if __name__ == "__main__":
    sys.exit(main())
