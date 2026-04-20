#!/usr/bin/env python3
"""
Extract individual entourage symbols from a composite top-down image
(typically an LLM-generated suite on a white background).

Given an input image, finds each isolated dark region, crops it with
padding, and emits one SVG per region that embeds the cropped PNG at a
real-world millimetre viewBox. Grove's EntourageLibrary reads the viewBox
to establish real-world scale, so the resulting files drop straight into
data/entourage/{category}/ and work identically to vectorised symbols.

Usage:
    python3 scripts/extract_entourage_from_image.py INPUT.jpg

The script prints a numbered list of detected regions and writes crops +
SVGs into scripts/.entourage-staging/. After running, edit
scripts/.entourage-staging/manifest.json to set category + name +
real-world mm per region, then re-run with `--finalise` to copy the
labelled SVGs into data/entourage/{category}/.
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import shutil
import sys
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
STAGING = REPO_ROOT / "scripts" / ".entourage-staging"
DATA_DIR = REPO_ROOT / "data" / "entourage"

MIN_COMPONENT_AREA_PX = 400   # dark-pixel count; low enough to keep thin 1-2px outlines
MIN_COMPONENT_SIDE_PX = 50    # bbox must be at least this wide/tall on one side
PADDING_PX = 8
THRESHOLD = 230               # pixels darker than this are "symbol"


@dataclass
class Region:
    index: int
    bbox: tuple[int, int, int, int]  # (x0, y0, x1, y1) in source pixels
    area_px: int


def detect_regions(image_path: Path) -> tuple[list[Region], "numpy.ndarray"]:
    import numpy as np
    from PIL import Image
    from scipy import ndimage

    img = Image.open(image_path).convert("L")
    arr = np.array(img)
    # Binary: True where the symbol is (darker than threshold)
    mask = arr < THRESHOLD

    # Label connected components (8-connectivity)
    structure = np.ones((3, 3), dtype=bool)
    labelled, n_labels = ndimage.label(mask, structure=structure)

    regions: list[Region] = []
    for label in range(1, n_labels + 1):
        ys, xs = np.where(labelled == label)
        if len(xs) < MIN_COMPONENT_AREA_PX:
            continue
        x0, x1 = xs.min(), xs.max() + 1
        y0, y1 = ys.min(), ys.max() + 1
        # Also require a minimum bbox side so we don't catch long thin specks
        if max(x1 - x0, y1 - y0) < MIN_COMPONENT_SIDE_PX:
            continue
        regions.append(Region(index=len(regions), bbox=(x0, y0, x1, y1), area_px=len(xs)))

    # Sort by (row-band, then column) so the numbering is reading-order
    regions.sort(key=lambda r: (r.bbox[1] // 80, r.bbox[0]))
    for i, r in enumerate(regions):
        r.index = i

    return regions, np.array(img.convert("RGB"))


def crop_with_padding(rgb, bbox: tuple[int, int, int, int], pad: int):
    import numpy as np
    from PIL import Image

    x0, y0, x1, y1 = bbox
    h, w = rgb.shape[:2]
    x0p = max(0, x0 - pad)
    y0p = max(0, y0 - pad)
    x1p = min(w, x1 + pad)
    y1p = min(h, y1 + pad)
    crop = rgb[y0p:y1p, x0p:x1p]
    return Image.fromarray(crop)


def white_to_alpha(crop_img, interior_fill: tuple[int, int, int] | None = None):
    """Make the OUTSIDE whitespace transparent, force outlines to pure
    crisp black, and leave the interior opaque white (or tinted).

    Algorithm:
      1. Classify each pixel by its darkest channel:
           - OUTLINE:  min < 130  → will become pure black (0,0,0,255)
           - LIGHT:    min > 150  → candidate for transparent or interior
           - MID-GREY: 130 ≤ min ≤ 150 → jagged AA fringe; resolved by
                                        neighbourhood (merged into outline)
      2. Flood-fill the LIGHT region from the image border. Components
         reached from the border = OUTSIDE.
      3. Write:
           OUTSIDE  → (0, 0, 0, 0) transparent
           OUTLINE  → (0, 0, 0, 255) crisp black
           else     → (255, 255, 255, 255) or `interior_fill`

    The hard thresholds kill the grey anti-aliasing halo that was
    showing as white nobbliness just outside the outline.
    """
    import numpy as np
    from PIL import Image
    from scipy import ndimage

    src = np.array(crop_img.convert("RGB"))
    h, w = src.shape[:2]

    r = src[:, :, 0].astype(np.int16)
    g = src[:, :, 1].astype(np.int16)
    b = src[:, :, 2].astype(np.int16)
    min_ch = np.minimum(np.minimum(r, g), b)

    outline_mask = min_ch < 130
    light_mask = min_ch > 150

    # Flood-fill: light pixels reachable from any image-border pixel.
    labelled, _ = ndimage.label(light_mask)
    edge_labels = set()
    for row in (0, h - 1):
        edge_labels.update(np.unique(labelled[row, :]).tolist())
    for col in (0, w - 1):
        edge_labels.update(np.unique(labelled[:, col]).tolist())
    edge_labels.discard(0)
    outside_mask = np.isin(labelled, list(edge_labels))

    out = np.zeros((h, w, 4), dtype=np.uint8)
    fill = interior_fill if interior_fill is not None else (255, 255, 255)

    # Default: interior (opaque, filled)
    out[:, :, 0] = fill[0]
    out[:, :, 1] = fill[1]
    out[:, :, 2] = fill[2]
    out[:, :, 3] = 255

    # Override with crisp black outline
    out[outline_mask] = [0, 0, 0, 255]

    # Override with transparent outside (overrides interior AND any outline
    # pixel that happened to fall inside an edge-connected light component,
    # which shouldn't happen but is safe).
    out[outside_mask] = [0, 0, 0, 0]

    return Image.fromarray(out, "RGBA")


def write_svg(crop_img, width_mm: float, height_mm: float, out_path: Path) -> None:
    """Embed the PNG crop in an SVG sized to the given mm dimensions.

    The crop is first run through `white_to_alpha` so the symbol sits on
    a transparent background rather than a white rectangle.
    """
    transparent = white_to_alpha(crop_img)
    buf = io.BytesIO()
    transparent.save(buf, format="PNG")
    b64 = base64.b64encode(buf.getvalue()).decode("ascii")

    svg = (
        f'<?xml version="1.0" encoding="UTF-8"?>\n'
        f'<svg xmlns="http://www.w3.org/2000/svg" '
        f'viewBox="0 0 {width_mm:g} {height_mm:g}">\n'
        f'  <image href="data:image/png;base64,{b64}" '
        f'x="0" y="0" width="{width_mm:g}" height="{height_mm:g}" '
        f'preserveAspectRatio="xMidYMid meet"/>\n'
        f'</svg>\n'
    )
    out_path.write_text(svg, encoding="utf-8")


def stage(image_path: Path) -> int:
    """Phase 1: detect, crop, write a blank manifest for the user to label."""
    STAGING.mkdir(parents=True, exist_ok=True)

    # Clear old staging
    for f in STAGING.glob("region_*.png"):
        f.unlink()

    regions, rgb = detect_regions(image_path)
    if not regions:
        print("no regions detected — is the image mostly white?", file=sys.stderr)
        return 1

    print(f"detected {len(regions)} regions")

    manifest_entries = []
    for r in regions:
        crop = crop_with_padding(rgb, r.bbox, PADDING_PX)
        crop_path = STAGING / f"region_{r.index:02d}.png"
        crop.save(crop_path)
        x0, y0, x1, y1 = r.bbox
        w_px = x1 - x0
        h_px = y1 - y0
        print(
            f"  #{r.index:02d}  bbox=({x0},{y0})-({x1},{y1})  "
            f"{w_px}×{h_px}px  area={r.area_px}px"
        )
        manifest_entries.append({
            "index": int(r.index),
            "bbox_px": [int(x0), int(y0), int(x1), int(y1)],
            "size_px": [int(w_px), int(h_px)],
            "crop_file": crop_path.name,
            # User fills these in:
            "category": "",           # people | vehicles | furniture | garden
            "name": "",               # display name, e.g. "Person Walking"
            "filename": "",           # destination .svg name
            "width_mm": 0,
            "height_mm": 0,
            "skip": False,
        })

    manifest_path = STAGING / "manifest.json"
    manifest_path.write_text(json.dumps(manifest_entries, indent=2))

    print(f"\nWrote {len(regions)} crops + manifest to {STAGING.relative_to(REPO_ROOT)}/")
    print(f"Edit {manifest_path.relative_to(REPO_ROOT)}, then re-run with --finalise")
    return 0


def finalise() -> int:
    """Phase 2: read the labelled manifest, emit SVGs into data/entourage/."""
    from PIL import Image

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
            continue  # not yet labelled
        if e["width_mm"] <= 0 or e["height_mm"] <= 0:
            print(f"  skipping #{e['index']:02d}: missing dimensions")
            continue

        crop_path = STAGING / e["crop_file"]
        if not crop_path.exists():
            print(f"  missing crop {crop_path}, skipping")
            continue
        crop = Image.open(crop_path)

        dest_dir = DATA_DIR / e["category"]
        dest_dir.mkdir(parents=True, exist_ok=True)
        dest = dest_dir / e["filename"]
        write_svg(crop, e["width_mm"], e["height_mm"], dest)
        written += 1
        print(f"  wrote {dest.relative_to(REPO_ROOT)}  ({e['width_mm']}×{e['height_mm']}mm)")

    print(f"\nfinalised {written} symbol(s)")
    return 0


def _parse_hex_rgb(s: str) -> tuple[int, int, int]:
    s = s.lstrip("#")
    if len(s) != 6:
        raise ValueError(f"need 6 hex digits, got {s!r}")
    return (int(s[0:2], 16), int(s[2:4], 16), int(s[4:6], 16))


def make_existing_transparent(interior_fill: tuple[int, int, int] | None = None) -> int:
    """Re-process raster-wrapped SVGs in data/entourage/ to make their
    embedded PNGs transparent. Idempotent — running twice is fine.

    If `interior_fill` is given, interior whites are tinted with that
    RGB. Otherwise they stay pure white.
    """
    import re
    from PIL import Image

    svgs = list(DATA_DIR.rglob("*.svg"))
    processed = 0
    skipped = 0
    for svg_path in svgs:
        text = svg_path.read_text(encoding="utf-8")
        m = re.search(r'href="data:image/png;base64,([A-Za-z0-9+/=]+)"', text)
        if not m:
            skipped += 1
            continue
        b64 = m.group(1)
        raw = base64.b64decode(b64)
        img = Image.open(io.BytesIO(raw))
        transparent = white_to_alpha(img, interior_fill=interior_fill)
        buf = io.BytesIO()
        transparent.save(buf, format="PNG")
        new_b64 = base64.b64encode(buf.getvalue()).decode("ascii")
        new_text = text.replace(b64, new_b64)
        svg_path.write_text(new_text, encoding="utf-8")
        processed += 1
        print(f"  transparented {svg_path.relative_to(REPO_ROOT)}")

    print(f"\nprocessed {processed} raster-wrapped SVG(s); {skipped} non-raster SVG(s) left alone")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[1])
    parser.add_argument("image", nargs="?", help="Input composite image")
    parser.add_argument("--finalise", action="store_true",
                        help="Emit labelled SVGs from an already-edited staging manifest")
    parser.add_argument("--retransparent-existing", action="store_true",
                        help="Re-process raster-wrapped SVGs in data/entourage/ to make "
                             "their backgrounds transparent")
    parser.add_argument("--interior-fill", default=None,
                        help="Hex RGB for interior whites (e.g. f5eedd for soft buff). "
                             "Default: leave interior pure white.")
    args = parser.parse_args()

    fill = _parse_hex_rgb(args.interior_fill) if args.interior_fill else None

    if args.retransparent_existing:
        return make_existing_transparent(interior_fill=fill)
    if args.finalise:
        return finalise()
    if not args.image:
        parser.error("provide an image path, --finalise, or --retransparent-existing")
    path = Path(args.image).expanduser()
    if not path.exists():
        print(f"not found: {path}", file=sys.stderr)
        return 1
    return stage(path)


if __name__ == "__main__":
    sys.exit(main())
