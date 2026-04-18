"""
Convert a FreeCAD .FCStd file into a top-down orthographic SVG sized in millimetres.

Invocation (inside FreeCAD console mode):
    FCSTD_INPUT=input.FCStd FCSTD_OUTPUT=output.svg [ENTOURAGE_MODE=hull|edges] \
        /path/to/FreeCAD.AppImage --console scripts/fcstd_to_svg.py

FreeCAD parses its own CLI flags first; when the final positional arg is a .py
file it runs the script in its interpreter. Inputs are passed via environment
variables because FreeCAD doesn't expose argv cleanly to scripts.

Two projection modes (controlled by ENTOURAGE_MODE):

  - hull (default): computes the convex hull of the solid's vertices projected
    onto the XY plane, then emits a single filled + stroked polygon. This gives
    a clean "footprint" outline — readable as a person or piece of furniture
    rather than a crosshatch of every 3D edge.

  - edges: the original behaviour — uses TechDraw.projectEx to emit every
    visible edge as a path. Useful for fountains / detailed architectural
    objects where the internal detail matters.

The output SVG uses viewBox "0 0 width_mm height_mm" so Grove's
EntourageLibrary picks up the correct real-world size automatically.
"""

import os
import sys
import traceback


def _convex_hull_xy(points):
    """Andrew's monotone chain convex hull for a list of (x, y) tuples.

    Returns the hull vertices in CCW order (without the final repeated point).
    O(n log n). Returns [] if fewer than 3 distinct points.
    """
    pts = sorted(set((round(x, 2), round(y, 2)) for x, y in points))
    if len(pts) < 3:
        return pts

    def cross(o, a, b):
        return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])

    lower = []
    for p in pts:
        while len(lower) >= 2 and cross(lower[-2], lower[-1], p) <= 0:
            lower.pop()
        lower.append(p)

    upper = []
    for p in reversed(pts):
        while len(upper) >= 2 and cross(upper[-2], upper[-1], p) <= 0:
            upper.pop()
        upper.append(p)

    return lower[:-1] + upper[:-1]


def main() -> int:
    input_path = os.environ.get("FCSTD_INPUT")
    output_path = os.environ.get("FCSTD_OUTPUT")
    mode = os.environ.get("ENTOURAGE_MODE", "hull").lower()
    if not input_path or not output_path:
        print("FCSTD_INPUT and FCSTD_OUTPUT must be set", file=sys.stderr)
        return 2

    try:
        import FreeCAD  # type: ignore
        import Part  # type: ignore
    except ImportError as e:
        print(f"not running inside FreeCAD: {e}", file=sys.stderr)
        return 2

    print(f"opening {input_path} (mode={mode})", flush=True)
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

    stroke_width = max(width, height) * 0.004

    svg_lines = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        f'<svg xmlns="http://www.w3.org/2000/svg" '
        f'viewBox="0 0 {width:g} {height:g}">',
    ]

    if mode == "hull":
        # Tessellate the fused solid into triangles, collect every triangle
        # vertex projected onto XY, then take the convex hull. This produces a
        # single clean footprint outline — much more legible than edge-salad.
        tessellation_tolerance = max(width, height) * 0.005  # 0.5% of size
        try:
            verts, _facets = combined.tessellate(tessellation_tolerance)
        except Exception as e:
            print(f"tessellate failed: {e}", file=sys.stderr)
            return 5

        projected = [(v.x, v.y) for v in verts]
        print(f"projected {len(projected)} vertices for hull", flush=True)
        hull = _convex_hull_xy(projected)
        if len(hull) < 3:
            print("hull has < 3 points — nothing to draw", file=sys.stderr)
            return 6

        # Translate to (0,0) origin, flip Y for SVG.
        pts_svg = [
            (x - bb.XMin, height - (y - bb.YMin)) for (x, y) in hull
        ]

        d = f"M {pts_svg[0][0]:.2f} {pts_svg[0][1]:.2f}"
        for x, y in pts_svg[1:]:
            d += f" L {x:.2f} {y:.2f}"
        d += " Z"
        svg_lines.append(
            f'<path d="{d}" fill="#e0e0e0" stroke="#404040" '
            f'stroke-width="{stroke_width:.2f}" '
            'stroke-linecap="round" stroke-linejoin="round"/>'
        )
        print(f"wrote hull with {len(hull)} vertices", flush=True)

    else:
        # Legacy "edges" mode: emit every visible edge from TechDraw projection.
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

        svg_lines.append(
            f'<g fill="none" stroke="#303030" stroke-width="{stroke_width:.2f}" '
            'stroke-linecap="round" stroke-linejoin="round">'
        )

        emitted = 0
        for edge in visible_edges.Edges:
            try:
                pts = edge.discretize(40)
            except Exception:
                continue
            if len(pts) < 2:
                continue
            xs = [p.x - bb.XMin for p in pts]
            ys = [height - (p.y - bb.YMin) for p in pts]

            if max(xs) - min(xs) < 0.01 and max(ys) - min(ys) < 0.01:
                continue

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
        print(f"wrote {emitted} edge paths", flush=True)

    svg_lines.append("</svg>")

    with open(output_path, "w", encoding="utf-8") as f:
        f.write("\n".join(svg_lines))
    print(f"wrote {output_path}", flush=True)
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
