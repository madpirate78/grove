"""
Convert a FreeCAD .FCStd file into a top-down orthographic SVG sized in millimetres.

Invocation (inside FreeCAD console mode):
    FCSTD_INPUT=input.FCStd FCSTD_OUTPUT=output.svg \
        /path/to/FreeCAD.AppImage --console scripts/fcstd_to_svg.py

FreeCAD parses its own CLI flags first; when the final positional arg is a .py
file it runs the script in its interpreter. Inputs are passed via environment
variables because FreeCAD doesn't expose argv cleanly to scripts.

The script:
  1. Opens the FCStd, collects all solid shapes.
  2. Fuses them into a single combined shape.
  3. Uses TechDraw.projectEx to project the combined shape onto the XY plane
     from +Z (a true top-down view).
  4. Writes an SVG whose viewBox is "0 0 width_mm height_mm", so Grove's
     EntourageLibrary picks up the correct real-world size automatically.
"""

import os
import sys
import traceback


def main() -> int:
    input_path = os.environ.get("FCSTD_INPUT")
    output_path = os.environ.get("FCSTD_OUTPUT")
    if not input_path or not output_path:
        print("FCSTD_INPUT and FCSTD_OUTPUT must be set", file=sys.stderr)
        return 2

    try:
        import FreeCAD  # type: ignore
        import Part  # type: ignore
    except ImportError as e:
        print(f"not running inside FreeCAD: {e}", file=sys.stderr)
        return 2

    print(f"opening {input_path}", flush=True)
    doc = FreeCAD.openDocument(input_path)

    shapes = []
    for obj in doc.Objects:
        shape = getattr(obj, "Shape", None)
        if shape is None or shape.isNull():
            continue
        if not shape.Solids:
            continue
        shapes.append(shape)

    if not shapes:
        print("no solid shapes found", file=sys.stderr)
        return 3

    print(f"fusing {len(shapes)} shape(s)", flush=True)
    combined = shapes[0]
    for s in shapes[1:]:
        try:
            combined = combined.fuse(s)
        except Exception as e:
            print(f"  fuse skipped: {e}", flush=True)

    bb = combined.BoundBox
    width = bb.XLength
    height = bb.YLength
    print(f"bbox: {width:.1f} x {height:.1f} mm", flush=True)

    if width <= 0 or height <= 0:
        print(f"invalid bbox {width}x{height}", file=sys.stderr)
        return 4

    try:
        import TechDraw  # type: ignore
        direction = FreeCAD.Vector(0, 0, 1)
        visible = TechDraw.projectEx(combined, direction)
        visible_edges = visible[0] if visible and visible[0] else None
    except Exception as e:
        print(f"TechDraw.projectEx failed: {e}", file=sys.stderr)
        return 5

    if visible_edges is None or not hasattr(visible_edges, "Edges"):
        print("no visible edges after projection", file=sys.stderr)
        return 6

    stroke_width = max(width, height) * 0.004

    svg_lines = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        f'<svg xmlns="http://www.w3.org/2000/svg" '
        f'viewBox="0 0 {width:g} {height:g}">',
        f'<g fill="none" stroke="#303030" stroke-width="{stroke_width:.2f}" '
        'stroke-linecap="round" stroke-linejoin="round">',
    ]

    emitted = 0
    for edge in visible_edges.Edges:
        try:
            pts = edge.discretize(40)
        except Exception:
            continue
        if len(pts) < 2:
            continue
        # Project to XY; flip Y because FreeCAD is Y-up, SVG is Y-down.
        xs = [p.x - bb.XMin for p in pts]
        ys = [height - (p.y - bb.YMin) for p in pts]

        # Filter degenerate paths (all points identical).
        if max(xs) - min(xs) < 0.01 and max(ys) - min(ys) < 0.01:
            continue

        # Deduplicate consecutive-identical points.
        cleaned_x = [xs[0]]
        cleaned_y = [ys[0]]
        for i in range(1, len(xs)):
            if abs(xs[i] - cleaned_x[-1]) > 0.01 or abs(ys[i] - cleaned_y[-1]) > 0.01:
                cleaned_x.append(xs[i])
                cleaned_y.append(ys[i])
        if len(cleaned_x) < 2:
            continue

        d = f"M {cleaned_x[0]:.2f} {cleaned_y[0]:.2f}"
        for i in range(1, len(cleaned_x)):
            d += f" L {cleaned_x[i]:.2f} {cleaned_y[i]:.2f}"
        svg_lines.append(f'<path d="{d}"/>')
        emitted += 1

    svg_lines.append("</g>")
    svg_lines.append("</svg>")

    with open(output_path, "w", encoding="utf-8") as f:
        f.write("\n".join(svg_lines))
    print(f"wrote {output_path} with {emitted} paths", flush=True)
    return 0


# FreeCAD imports this file as a module (__name__ == "fcstd_to_svg"), so the
# usual `if __name__ == "__main__":` guard would prevent execution. Run main()
# unconditionally at import time.
try:
    rc = main()
except Exception:
    traceback.print_exc()
    rc = 1
sys.exit(rc)
