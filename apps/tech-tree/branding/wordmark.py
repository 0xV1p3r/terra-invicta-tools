# Builds wordmark-dark.svg and wordmark-light.svg: the icon next to "Terra Invicta Tools" set in
# Unbounded Bold, with the text converted to outlines so the SVGs need no font.
# Usage, from apps/tech-tree/branding/ with Unbounded[wght].ttf from Google Fonts:
#   uv run --no-project --with fonttools python wordmark.py Unbounded[wght].ttf icon.svg .
import re, sys
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen

font_path, icon_path, out_dir = sys.argv[1:4]
font = instantiateVariableFont(TTFont(font_path), {"wght": 700})
upm = font["head"].unitsPerEm
cap = font["OS/2"].sCapHeight
gs = font.getGlyphSet()
cmap = font.getBestCmap()
hmtx = font["hmtx"]

H = 180
CAP_PX = 50
scale = CAP_PX / cap

def text_path(s, x0, baseline):
    pen = SVGPathPen(gs)
    x = 0
    for ch in s:
        g = cmap[ord(ch)]
        gs[g].draw(TransformPen(pen, (scale, 0, 0, -scale, x0 + x * scale, baseline)))
        x += hmtx[g][0]
    return pen.getCommands(), x * scale

icon = open(icon_path).read()
icon_body = re.sub(r"(?s)^.*?<title>.*?</title>\s*|</svg>\s*$", "", icon)

tx = H + 40
gap = 22
top = (H - (2 * CAP_PX + gap)) / 2
l1, w1 = text_path("Terra Invicta", tx, top + CAP_PX)
l2, w2 = text_path("Tools", tx, top + 2 * CAP_PX + gap)
W = round(tx + max(w1, w2) + 4)

def lockup(c1, c2):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}" role="img" aria-label="Terra Invicta Tools">\n'
            f'<title>Terra Invicta Tools</title>\n'
            f'<g transform="scale({H / 512:.6f})">\n{icon_body.strip()}\n</g>\n'
            f'<path fill="{c1}" d="{l1}"/>\n<path fill="{c2}" d="{l2}"/>\n</svg>\n')

open(f"{out_dir}/wordmark-dark.svg", "w").write(lockup("#8e919a", "#f3f3f4"))
open(f"{out_dir}/wordmark-light.svg", "w").write(lockup("#5c6069", "#101113"))
print(W, H)
