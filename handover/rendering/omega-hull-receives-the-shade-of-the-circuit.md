---
status: measure
titles: [omega]
updated: 2026-11-27
summary: "Omega's hull shade is wired under original; open: sunlit-panel term, slot 8 housing, fit rule, bias, default tier"
categories: [rendering]
---

# Omega's hull takes the shade of the circuit from a map of its own

2026-11-27, lane `hull-shadow-wire`. The wiring is complete: `HullShade::CircuitAndSelf { reach: 377.0 }` on Omega,
under `--shadows original` only. The values, the chosen constants and the measurement are in
[track-shadow.md](../../docs/ghidra/functions/ps4-omega-eu/track-shadow.md), "Wired: Omega's hull takes the shade of the
circuit". On the pole slot the acceptance holds. On slot 8 two numbers do not agree with the original.

## Open

- The original has an additive term on sunlit panels that ours lacks. The hull's reflection is the candidate. It is why
  slot 8's sunlit-panel ratio does not agree: ours 0.86 to 1.0, the original 0.49 to 0.64.
- Slot 8's housing reads 0.21 to 0.23 on ours against 0.10 to 0.12 on the original. It is 0.11 to 0.13 on a tighter box.
- The fit of the box and of its first two axes to the craft is not read. Ours' 7 units each way is chosen, not measured.
- The hull draw's own `shadowCoeffs` values (map size, three bias terms) are not read. Ours' bias of 0.0002 is chosen,
  not measured.
- Whether the original queues circuit nodes for every craft or for the first alone is not read. Ours draws every craft.
- The two floats that choose between caster bits `0x2000` and `0x4000` are not named. Modesto uses `0x2000`, and
  every node there has both.
- The `CastNoShipShadows` tag is not parsed. Ours stands in with "transparent draws do not cast", which is chosen, not
  measured.
- Omega's default tier stays `off`, so a default player sees no change. Flipping it is the maintainer's call.
- It moves every adopted Omega pair under `original` (the hull first). Agree that with the maintainer before the merge.
- 2048: not checkable: no 2048 package on the machine.
- HD and 2048 pixel identity under `original` holds by construction and is not measured here: no HD image and no 2048
  package.

## Next Steps

1. Read what adds to the original's sunlit panels in both knockout states. Build the hull's reflection if it is that.
   Then repeat the slot 8 measurement. `scripts/wipeout-pc-replay-shadow-live.py` and
   `scripts/wipeout-pc-replay-shadow-ratio.py` repeat the original's side.
2. Read the fit rule and the hull draw's `shadowCoeffs`, and replace the chosen box and bias.
3. Ask the maintainer about the default tier and the adopted pairs.
