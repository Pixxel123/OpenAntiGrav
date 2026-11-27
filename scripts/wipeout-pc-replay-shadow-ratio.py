#!/usr/bin/env python3
"""The original's shadow term per box from `wipeout-pc-replay-shadow-live.py`'s frames: `A / (A + B)` of the two knockout
states, on sRGB-decoded means, pixels at the top of the range left out.

    uv run --with numpy --with pillow python3 scripts/wipeout-pc-replay-shadow-ratio.py <dir with note.json and the frames> \\
        housing=590,470,690,560 spine=610,410,670,450 track=340,360,440,410

The frames are the port's screenshots, named `..._HHMMSS_...png`. The first frame of a state is dropped.
`docs/ghidra/functions/ps4-omega-eu/hull-tone.md`, "The shadow term, read by knockout".
"""
import json, re, sys
from pathlib import Path
import numpy as np
from PIL import Image

root = Path(sys.argv[1])
boxes = {k: tuple(int(x) for x in v.split(",")) for k, v in (a.split("=") for a in sys.argv[2:])}
note = json.loads((root / "note.json").read_text())
names = sorted(p.name for p in root.glob("*.png") if re.search(r"_\d{6}_", p.name))


def decoded(path):
    a = np.asarray(Image.open(path).convert("RGB"), dtype=np.float64) / 255
    return np.where(a <= 0.04045, a / 12.92, ((a + 0.055) / 1.055) ** 2.4), a


state = {}
for s in note["states"]:
    frames = [n for n in names if s["from"] < re.search(r"_(\d{6})_", n).group(1) <= s["to"]]
    frames = frames[1:] if len(frames) > 2 else frames
    pairs = [decoded(root / f) for f in frames]
    state[s["tag"]] = (np.mean([p[0] for p in pairs], axis=0), np.max([p[1] for p in pairs], axis=0), len(frames))
    print(s["tag"], len(frames), "frames")
a, top_a, _ = state["sA-sun0-K4-Ks0"]
b, top_b, _ = state["sB-sun0-K0-Ks4"]
for name, (x0, y0, x1, y1) in boxes.items():
    ok = (top_a[y0:y1, x0:x1].max(-1) < 0.98) & (top_b[y0:y1, x0:x1].max(-1) < 0.98)
    am, bm = a[y0:y1, x0:x1][ok].mean(0), b[y0:y1, x0:x1][ok].mean(0)
    rest = {t: np.round(v[0][y0:y1, x0:x1].mean((0, 1)), 3) for t, v in state.items() if t.startswith(("s0", "sC"))}
    print(name, "in range %d%%" % (100 * ok.mean()), "A", np.round(am, 3), "B", np.round(bm, 3), "shadow", np.round(am / (am + bm), 2), rest)
