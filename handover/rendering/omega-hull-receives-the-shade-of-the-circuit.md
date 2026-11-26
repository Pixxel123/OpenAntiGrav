---
status: ready-to-wire
titles: [omega]
updated: 2026-11-26
summary: "Omega's hull reads a 512 depth map of its own that holds nearby circuit pieces and the craft; ours has no such shade; wire it by title data"
categories: [rendering]
---

# Omega's hull takes the shade of the circuit from a map of its own

2026-11-26, lane `hull-shadow-re`. Evidence:
[track-shadow.md](../../docs/ghidra/functions/ps4-omega-eu/track-shadow.md), "The circuit's pieces cast onto craft",
and [hull-tone.md](../../docs/ghidra/functions/ps4-omega-eu/hull-tone.md), "The shadow term, read by knockout".

The original draws a map for each craft. The map is orthographic along `Lighting.Sun direction` and is 512 by 512.
Its box is about 10 by 9.5 units, fitted to the craft. It is 377 units deep, with the far plane at the craft.

Its casters are the circuit nodes near the craft that have the caster bit, then the craft's own model. The hull
compares against the map with nine taps and a slope bias. The result is the `shadow` of the hull's law: it gates
the sun term and the `K` to `Ks` mix.

Measured on the port, Modesto Heights, classic Feisar:

| Panel | `shadow`, pole slot (in the shade) | `shadow`, grid slot 8 |
| --- | --- | --- |
| spine | `0.01` | `0.36` to `0.42` |
| housing bay | `0.08` | `0.10` |

With the caster bits cleared, the spine on the pole slot goes to `0.27`.

Ours: with `--shadows off` (Omega's default) the hull has no shade. With `original` it has HD's two maps: the
craft alone as a caster, times HD's road mask. Neither is Omega's.

## Open

- The fit of the box and of its first two axes to the craft is not read. A fit to the craft's bounds in light
  space is a choice, and the code must say so.
- The hull draw's own `shadowCoeffs` values (map size, three bias terms) are not read. Ours' bias is a choice.
- Whether the original queues circuit nodes for every craft or for the first alone is not read. One stopped frame
  showed the query's entry for the first alone.
- The two floats that choose between caster bits `0x2000` and `0x4000` are not named. Modesto uses `0x2000`, and
  every node there has both.
- 2048: not checked, no 2048 package was on the machine. The 2048 notes describe the same shape
  (`docs/rendering/shadows.md`, "2048's and Omega's `original`").
- It moves every adopted Omega pair (the hull first). Agree that with the maintainer before the merge.

## Next Steps

1. Give `oag-omega` title data for the pass.
   - Casters: the craft, and the circuit draws in the craft's light column.
   - Not casters: a material that binds `screenSpaceRefractionTex`, and a node tagged `CastNoShipShadows`.
   - Fit: along the sun, 377 deep, far plane at the craft.
   - Lookup: the depth map alone, with no road mask.
2. Reuse the per-craft depth map of `oag_render::shadow::self_shadow` (512 texels already). Add the circuit draws
   as casters for a title that asks for them. HD and 2048 must stay pixel-identical.
3. Take nine taps on Omega (three by three, compared).
4. Acceptance: ours' `shadow` on Modesto Heights. Use the two knockout states on ours' side: the sun at 0 with the
   ambient at `K` alone, then at `Ks` alone.
   - The spine is within `0.1` of `0.01` on the pole slot, and of `0.36` to `0.42` on slot 8.
   - The housing bay is under `0.2` on both slots.
   - `scripts/wipeout-pc-replay-shadow-live.py` and `scripts/wipeout-pc-replay-shadow-ratio.py` repeat the
     original's side.
5. Then redo the engine light's warm-pixel census
   (`docs/ghidra/functions/ps4-omega-eu/engine-light.md`, "Ours at a forced range"). The unlit excess should fall.
