#!/usr/bin/env python3
"""
Extract individual entourage symbols as TRUE VECTOR SVGs from a
composite input. Accepts either:
  - a PNG/JPG (typically an LLM-generated composite), which is traced
    automatically using the pure-Python potrace port (`potracer` on PyPI,
    imported as `potrace`), or
  - an SVG (e.g. from Inkscape's trace-bitmap dialog).

Output is one small standalone SVG per detected item, each with a
viewBox in real-world millimetres.

Usage:
    python3 scripts/extract_entourage_from_svg.py INPUT.(png|jpg|svg)

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
INKSCAPE_PREFS = Path.home() / ".config" / "inkscape" / "preferences.xml"

DEFAULT_SIMPLIFY_THRESHOLD = 0.0005

# How close two subpath bboxes need to be (relative to their size) to
# be considered part of the same item. 0 means only touching.
# Cluster gap is computed dynamically at runtime as a fraction of the source
# coordinate space (e.g. 1% of the longer dimension). A 1408×768 pixel trace
# gets ~14-unit gap; a 297×210 mm Inkscape SVG gets ~3-unit gap.
CLUSTER_GAP_FRACTION = 0.01


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


def find_top_level(subpaths: list[Subpath], indices: list[int]) -> list[int]:
    """Return the subset of `indices` whose bboxes are not fully contained
    inside any other subpath in the same set. These are the outer contours
    of the item and together define its silhouette."""
    bboxes = [subpaths[i].bbox for i in indices]
    top = []
    for a, ia in enumerate(indices):
        ba = bboxes[a]
        contained = False
        for b, ib in enumerate(indices):
            if a == b:
                continue
            bb = bboxes[b]
            if bb[0] <= ba[0] and bb[1] <= ba[1] and bb[2] >= ba[2] and bb[3] >= ba[3]:
                if bb != ba:  # strict containment
                    contained = True
                    break
        if not contained:
            top.append(ia)
    return top


def _set_simplify_threshold(value: float) -> str | None:
    """Write `value` into Inkscape's simplifythreshold preference.
    Returns the previous value so the caller can restore it."""
    if not INKSCAPE_PREFS.exists():
        return None
    text = INKSCAPE_PREFS.read_text(encoding="utf-8")
    m = re.search(r'(id="simplifythreshold"[^/]*?value=")([^"]+)(")', text, re.DOTALL)
    if not m:
        return None
    old = m.group(2)
    new_text = text[: m.start(2)] + f"{value}" + text[m.end(2) :]
    INKSCAPE_PREFS.write_text(new_text, encoding="utf-8")
    return old


def _restore_simplify_threshold(old_value: str | None) -> None:
    if old_value is None or not INKSCAPE_PREFS.exists():
        return
    text = INKSCAPE_PREFS.read_text(encoding="utf-8")
    text = re.sub(
        r'(id="simplifythreshold"[^/]*?value=")([^"]+)(")',
        lambda m: m.group(1) + old_value + m.group(3),
        text, count=1, flags=re.DOTALL,
    )
    INKSCAPE_PREFS.write_text(text, encoding="utf-8")


def inkscape_simplify(svg_path: Path, threshold: float) -> None:
    """Run Inkscape's path-simplify action in-place, temporarily setting
    the simplifythreshold preference to `threshold`."""
    import subprocess
    old = _set_simplify_threshold(threshold)
    try:
        subprocess.run(
            [
                "inkscape",
                str(svg_path),
                "--actions",
                f"select-all;path-simplify;export-filename:{svg_path};export-do",
            ],
            capture_output=True,
            timeout=60,
            check=False,
        )
    finally:
        _restore_simplify_threshold(old)


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


def trace_image_to_d(image_path: Path, threshold: int = 200) -> tuple[str, float, float]:
    """Binarise an image (dark pixels = ink) and run potrace to return
    a single composite `d` attribute plus the image dimensions."""
    import potrace  # type: ignore
    import numpy as np
    from PIL import Image

    img = Image.open(image_path).convert("L")
    arr = np.array(img)
    h_px, w_px = arr.shape
    mask = arr < threshold

    bmp = potrace.Bitmap(mask)
    path = bmp.trace()

    img_area = w_px * h_px
    parts: list[str] = []
    kept = 0
    for curve in path:
        sp = curve.start_point
        xs = [sp.x]
        ys = [sp.y]
        seg_strs = [f"M {sp.x:.2f} {sp.y:.2f}"]
        for seg in curve:
            if seg.is_corner:
                seg_strs.append(
                    f"L {seg.c.x:.2f} {seg.c.y:.2f} "
                    f"L {seg.end_point.x:.2f} {seg.end_point.y:.2f}"
                )
                xs += [seg.c.x, seg.end_point.x]
                ys += [seg.c.y, seg.end_point.y]
            else:
                seg_strs.append(
                    f"C {seg.c1.x:.2f} {seg.c1.y:.2f} "
                    f"{seg.c2.x:.2f} {seg.c2.y:.2f} "
                    f"{seg.end_point.x:.2f} {seg.end_point.y:.2f}"
                )
                xs += [seg.c1.x, seg.c2.x, seg.end_point.x]
                ys += [seg.c1.y, seg.c2.y, seg.end_point.y]
        seg_strs.append("Z")
        # Drop the image-bounding rect potrace adds as its outer contour.
        bbox_area = (max(xs) - min(xs)) * (max(ys) - min(ys))
        if bbox_area > 0.5 * img_area:
            continue
        parts.append(" ".join(seg_strs))
        kept += 1
    if not parts:
        raise RuntimeError("trace produced no usable curves")
    print(f"traced {kept} curves from {image_path.name} ({w_px}x{h_px}px)")
    return " ".join(parts), float(w_px), float(h_px)


def translate_subpath(sub: str, start_abs: tuple[float, float], dx: float, dy: float) -> str:
    """Rewrite a subpath so its coordinates are shifted by -dx, -dy.

    Traverses every command token. Absolute commands have their
    x/y coordinates shifted. Relative commands pass through unchanged
    (since their deltas don't depend on absolute origin). The leading
    M/m is always replaced with an absolute M at (start_abs - delta).
    """
    out_parts: list[str] = []
    first = True
    for cmd, nums in _tokens(sub):
        if first:
            first = False
            nx = start_abs[0] - dx
            ny = start_abs[1] - dy
            out_parts.append(f"M {nx:.3f},{ny:.3f}")
            # If the leading M/m had continuation pairs, re-emit those.
            if len(nums) > 2:
                rest = nums[2:]
                if cmd == "M":
                    # Implicit absolute L's
                    for i in range(0, len(rest) - 1, 2):
                        out_parts.append(f"L {rest[i] - dx:.3f},{rest[i+1] - dy:.3f}")
                else:
                    # Implicit relative l's — unchanged
                    for i in range(0, len(rest) - 1, 2):
                        out_parts.append(f"l {rest[i]:.3f},{rest[i+1]:.3f}")
            continue

        if cmd.isupper():
            # Absolute: shift each coordinate
            c = cmd
            if c in ("M", "L", "T"):
                # pairs of (x, y)
                shifted = []
                for i in range(0, len(nums) - 1, 2):
                    shifted.append(f"{nums[i] - dx:.3f},{nums[i+1] - dy:.3f}")
                out_parts.append(f"{c} " + " ".join(shifted))
            elif c == "H":
                out_parts.append("H " + " ".join(f"{n - dx:.3f}" for n in nums))
            elif c == "V":
                out_parts.append("V " + " ".join(f"{n - dy:.3f}" for n in nums))
            elif c == "C":
                shifted = []
                for i in range(0, len(nums) - 5, 6):
                    shifted.append(
                        f"{nums[i]-dx:.3f},{nums[i+1]-dy:.3f} "
                        f"{nums[i+2]-dx:.3f},{nums[i+3]-dy:.3f} "
                        f"{nums[i+4]-dx:.3f},{nums[i+5]-dy:.3f}"
                    )
                out_parts.append("C " + " ".join(shifted))
            elif c == "S" or c == "Q":
                shifted = []
                for i in range(0, len(nums) - 3, 4):
                    shifted.append(
                        f"{nums[i]-dx:.3f},{nums[i+1]-dy:.3f} "
                        f"{nums[i+2]-dx:.3f},{nums[i+3]-dy:.3f}"
                    )
                out_parts.append(f"{c} " + " ".join(shifted))
            elif c == "A":
                # rx ry xrot large sweep x y — shift only x,y
                shifted = []
                for i in range(0, len(nums) - 6, 7):
                    shifted.append(
                        f"{nums[i]} {nums[i+1]} {nums[i+2]} "
                        f"{int(nums[i+3])} {int(nums[i+4])} "
                        f"{nums[i+5]-dx:.3f},{nums[i+6]-dy:.3f}"
                    )
                out_parts.append("A " + " ".join(shifted))
            elif c == "Z":
                out_parts.append("Z")
            else:
                out_parts.append(cmd + " " + " ".join(f"{n:g}" for n in nums))
        else:
            # Relative — passes through (deltas don't depend on origin)
            if cmd == "z":
                out_parts.append("z")
            else:
                out_parts.append(cmd + " " + " ".join(f"{n:g}" for n in nums))
    return " ".join(out_parts)


def stage(input_path: Path) -> int:
    STAGING.mkdir(parents=True, exist_ok=True)
    for f in STAGING.glob("vec_*.svg"):
        f.unlink()

    suffix = input_path.suffix.lower()
    if suffix in {".png", ".jpg", ".jpeg"}:
        d, vb_w, vb_h = trace_image_to_d(input_path)
    elif suffix == ".svg":
        svg_text = input_path.read_text(encoding="utf-8")
        d, vb_w, vb_h = extract_path_d(svg_text)
    else:
        print(f"unsupported input type: {suffix}", file=sys.stderr)
        return 1

    subpaths = parse_all_subpaths(d)
    # Drop degenerate subpaths
    subpaths = [s for s in subpaths if s.bbox[2] > s.bbox[0] and s.bbox[3] > s.bbox[1]]
    print(f"parsed {len(subpaths)} non-degenerate subpaths")

    gap = max(vb_w, vb_h) * CLUSTER_GAP_FRACTION
    print(f"cluster gap: {gap:.2f} (source viewport {vb_w:g}×{vb_h:g})")
    clusters = cluster_subpaths(subpaths, gap=gap)
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

        # Top-level (outermost) curves define the silhouette — fill them
        # white so the symbol's interior is opaque white rather than
        # transparent. All curves together are the ink, filled dark with
        # fill-rule="evenodd" so holes (windows, wheel wells) punch through.
        top_indices = find_top_level(subpaths, cluster)
        silhouette_pieces = []
        for si in top_indices:
            moved = translate_subpath(subpaths[si].source, subpaths[si].start_abs, vb_x, vb_y)
            silhouette_pieces.append(moved)
        ink_pieces = []
        for si in cluster:
            moved = translate_subpath(subpaths[si].source, subpaths[si].start_abs, vb_x, vb_y)
            ink_pieces.append(moved)
        d_silhouette = " ".join(silhouette_pieces)
        d_ink = " ".join(ink_pieces)

        svg = (
            f'<?xml version="1.0" encoding="UTF-8"?>\n'
            f'<svg xmlns="http://www.w3.org/2000/svg" '
            f'viewBox="0 0 {vb_width:g} {vb_height:g}">\n'
            f'  <path d="{d_silhouette}" fill="#ffffff" fill-rule="evenodd" stroke="none"/>\n'
            f'  <path d="{d_ink}" fill="#2a2a2a" fill-rule="evenodd" stroke="none"/>\n'
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


def finalise(simplify_threshold: float = DEFAULT_SIMPLIFY_THRESHOLD) -> int:
    """Read the labelled manifest and rewrite each vec_XX.svg with the
    real-world mm viewBox into data/entourage/{category}/{filename}.

    If `simplify_threshold > 0`, run Inkscape's path-simplify over each
    output SVG at that threshold (overrides the user's Inkscape pref
    temporarily and restores it afterwards).
    """
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
        if simplify_threshold > 0:
            inkscape_simplify(dest, simplify_threshold)
        written += 1
        print(f"  wrote {dest.relative_to(REPO_ROOT)}  ({e['width_mm']}×{e['height_mm']}mm)")
    print(f"\nfinalised {written} vector symbol(s)  (simplify={simplify_threshold})")
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[1])
    p.add_argument("input", nargs="?", help="Input composite PNG/JPG/SVG")
    p.add_argument("--finalise", action="store_true")
    p.add_argument("--simplify", type=float, default=DEFAULT_SIMPLIFY_THRESHOLD,
                   help=f"Inkscape path-simplify threshold (0 to disable). "
                        f"Default: {DEFAULT_SIMPLIFY_THRESHOLD}")
    args = p.parse_args()
    if args.finalise:
        return finalise(simplify_threshold=args.simplify)
    if not args.input:
        p.error("provide an image/SVG path, or --finalise")
    path = Path(args.input).expanduser()
    if not path.exists():
        print(f"not found: {path}", file=sys.stderr)
        return 1
    return stage(path)


if __name__ == "__main__":
    sys.exit(main())
