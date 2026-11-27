# The track shadow `S`: three exponential cascades cast along the sun, and the sun term they remove

2026-11-20, `omega-shadow-map`. **Every address on this page is the 1.07 patch eboot's** (Ghidra program
`/omega/eboot-ps4-omega-eu-v107.bin`), not the base eboot the rest of this directory and its `names.tsv` use
([patch-v107-addresses](patch-v107-addresses.md)). None of these functions has been placed in the base program, so this page
renames nothing and adds **no `names.tsv` row**: a row with a 1.07 address would replay onto unrelated bytes in the base import.
The live reads and the knockouts are on the `wipeout_pc` native port (the 1.07 eboot run natively) through
`scripts/wipeout-pc-shadow-live.py`.

**What `S` is, in one paragraph.** Each frame the craft (and whatever else is flagged a caster) are drawn into three
orthographic depth maps looking along the circuit's **`Lighting.Sun direction`**, the cascades fitted to the casters' bounding
spheres within three camera-depth bands. The depth is turned into an exponential shadow map with exponent 80. A receiver
evaluates `S = min(1, pcf(shadowMap) * exp2(clamp(c0 + c1 z, -80, 0)))` and multiplies **only its sun terms** (the diffuse
`sun * LM.a * N.L` and the sun lobe) by it. Live, inside a craft's shadow the sun term is gone entirely: `S` reaches 0, with no
residual strength.

## The parts, with confidence

| Part | Law | Evidence | Confidence |
| --- | --- | --- | ---: |
| Cast axis | `D = normalize(Lighting.Sun direction)`, the same vector as `liveLighting0direction` | static: `EnvSettings_RegisterKeys` (1.07 `FUN_015eda20`) binds `0x01eb47d0` to `Lighting.Sun direction` (`015eda81`, string `0x0189140a`); `Scene_PrepareFrame` (1.07 `FUN_015f2d30`) normalises it at `015f7380`-`015f7456` and keeps it at `[RBP-0x290]` (`015f7487`), the light basis is built from it at `015f7d65`-`015f82dd`. `ShadowLight direction` (`0x01eb47e0`, string `0x0188d035`) is normalised beside it (`015f7388`) and is not the one stored. Live (Capital Reach, three boots): the light view's forward column `0x0221af68/78/88` reads `(0.1867, 0.5000, 0.8457)`, equal to `liveLighting0direction` `(0.18675, 0.5, 0.84565)` and to the pushed direction `0x0221ac20`; `shadowLightDirection` is `(-0.310, 0.706, -0.637)` | 90 |
| Light eye | the camera position plus `300 * D` (`0x01856140`, `015f80b6`), looking back along `-D` | static | 70 |
| Caster push | each caster's sphere centre moves by `-k * D`, `k = clamp(2.5 / max(abs(D.y), 1e-4), 4, 8)` (`0x01863284` 2.5, `0x01863280` 1e-4, `0x01863288` 8, `0x0186328c` 4; stored negated at `0x019a81d4`, `015f7dab`) | static; live `-5.0` on Capital Reach (`D.y = 0.5`) | 85 |
| Cascades | three, by the camera-view depth of a caster's sphere: `[0.5, 38.9]`, `[34.1, 110.9]`, `[101.3, 254.9]` (`0x01856110` and the immediates `0x42ca999a`, `0x437ee667` at `015f8076`/`015f8080`); each cascade's light-space rectangle is the min/max of the spheres that overlap its band (`FUN_01670960`, called at `015f863b` and `015f89d3`) | static; live, the six splits at `0x020e4c60` and three filled rectangles at `0x020e4c20`/`0x020e4c40` on Capital Reach | 80 |
| Casters | (1) the frame's model instance list (`0x01edc810` count, `0x01edb240` indices into 0x360-byte records at `0x01edf3e0`): a record with bit 1 of `+0x110` set, its sphere at `+0xf0` (radius plus 1.0), culled against six planes; (2) a second list under `*0x020e4268`, gated by `0x020e4340 == 1` and a flag on `*0x01a8ac98` (the gate read 0 on Capital Reach) | static; live, only craft-shaped shadows were seen. Which models carry the bit, and what the second list holds, is not read. `CastShipShadows` / `CastNoShipShadows` are read only inside `FUN_0162a390` (`0162e1f1`), another lane's function, and were not followed | 55 |
| Map encoding | exponential shadow map: the depth pass is converted with the exponent **80.0** (`0x42a00000`, written into the conversion pass's constants in `FUN_01670ba0`) against the near and far `0x020e4bb8`/`0x020e4bb4` (set by the shadow object's method `FUN_01670930`) | static | 70 |
| `shadowCoeffs` | built per frame at `015f9260`-`015f94a4` into a scratch block, its pointer at `0x019a8070`, bound to slot 104 (bank `+0x1050`, `015fd9ed`; name-table hash `0x0ae03ae7` at `+0x1040`). Rows 0 and 1: cascade `k` to `k + 1` remaps (`(c_k - c_k1) / w_k1`, `w_k / w_k1` per axis). Row 2: the two band fades, `(far_k - 0.25 * overlap, 2 / overlap)` (`37.7, 0.4167, 108.5, 0.2083`). Row 3: `(c0, c1, -80, 0)`, `c0` and `c1` scaled by the literals `-80.0` (`0x018632c0`) and `-160.0` (`0x018632c4`) | static; live on Capital Reach row 3 `(168.0, -799.2, -80, 0)`, on Sol `(-40.8, -799.2, -80, 0)` (`panel-displacement.md`); rows 2 equal on both | 85 |
| `shadowMatrix` | slot 103 (hash `0x6c1a1be7` at `+0x1018`, bound at `015fe14a`) | live value read, not decomposed | 60 |
| Receiver law | `S = min(1, pcf(shadowMap) * exp2(clamp(c0 + c1 z, -80, 0)))` on the sun diffuse and the sun lobe only | the tarmac program `@0xa764` (`omega-status.md`, "The tarmac roads' lit programs"); live knockouts below | 85 |
| How dark | inside a craft's shadow `S` reaches 0: the shadowed pixels equal the frame with every `S` forced to 0, to 0.85 to 1.04 of the sun term over five shadows on Metro Park | live, one boot | 80 |

## The knockouts, live

`Scene_PrepareFrame` is the only reader of the literals `-80.0` and `-160.0` at `0x018632c0`/`0x018632c4` (a whole-program operand
search finds one instruction each). Two writes to this run's own port process, each undone after its frame:

- **`S = 1`**: both literals written `0`: row 3 becomes `(0, 0, 0, 0)` (read back from the bound bank while the write stood), so
  `S = min(1, pcf(map)) = 1` (the map holds `exp(80 d) >= 1`). Every map shadow is removed.
- **`S = 0`**: `-160.0` written `-1.6e6`, so `c1` reads `-7.99e6` and the exponent sits at its floor: every receiver loses its
  sun term. Recovery after the write is undone is within 0.24 s (the frame grabbed after it).

The port maps the eboot privately: after all four boots `eboot.bin` still hashes `3a52e910...f644c`, the patch's own.

| Circuit, region (1920x1080, mean grey) | shadow on | `S = 1` | `S = 0` |
| --- | ---: | ---: | ---: |
| Capital Reach, sunlit streak `400x100+400+420` | 154.2 | 152.5 | 95.6 |
| Capital Reach, whole frame | 95.6 | 95.4 | 95.0 |
| Metro Park, five craft shadows (masked where `S = 1` brightens by 15 or more) | 62.7 to 159.9 | 109.6 to 239.8 | 60.9 to 146.0 |
| Metro Park, grass left `400x300+0+700` | 113.4 | 114.2 | 75.1 |
| Metro Park, hull top `200x140+860+560` | 106.7 | 107.5 | 90.4 |
| Metro Park, whole frame | 116.4 | 116.9 | 101.2 |

What this settles:

1. **The tarmac binds the shadow-map permutation** (`@0xa764`): forcing `S = 0` takes Capital Reach's sunlit streaks from 154 to
   96 grey. `omega-status.md`'s confidence-70 assumption is now measured (confidence 90).
2. **Capital Reach's grid shows no map shadow**: `S = 1` changes no region beyond frame-to-frame noise in two boots (three rounds
   in the first). The sun there reaches the road only in the baked `LM.a` streaks, and no craft stands in one. Capital Reach's
   whole-frame gap is therefore not `S` (its whole sun term is 0.6 grey of the frame).
3. **Metro Park's grid shows the mechanism**: every craft on the sunlit brick and grass casts a shadow that `S = 1` removes, and
   inside it the frame equals the `S = 0` frame (the sun term is removed whole).
4. **The hull receives `S` too** (hull top 106.7 to 90.4 under `S = 0`), which is the `shadow` weight of
   [hull-tone.md](hull-tone.md)'s `(Ks + (K - Ks) * shadow)`. The grass (`grassmudrock_blend_...`) receives it as well.

## 2048: checked, differs

2048 casts its craft map along `Lighting.Ship shadow direction` into one map fitted to the frustum out to 60 units
([ship-shadows.md](../vita-2048-eu-v104/ship-shadows.md)). Omega casts along `Lighting.Sun direction` into three exponential
cascades out to 254.9 and has neither of 2048's debug gates. The receiver's form (a map term on the sun) is the shared lineage;
the axis, the fit and the encoding differ.

## The circuit's pieces cast onto craft: `CastShipShadows` (2026-11-26)

Read in the 1.07 eboot for the hull's shade ([hull-tone.md](hull-tone.md), "The shadow term, read
by knockout"). The addresses are the 1.07 program's.

**Each node of a circuit model is marked as a caster onto craft unless it says otherwise
(confidence 70).** The circuit loader `FUN_0162a390` does this for each node, at `0x0162e1e7`:

1. It sets bits `0x6000` of the word at `+0xa0` of the node's record (`*pbVar |= 0x60` at `+0xa1`).
2. If the node has the tag `CastShipShadows`, the bits stay.
3. If not, and the node has the tag `CastNoShipShadows`, it clears the bits (`&= 0xffff9fff`).
4. If the node has neither tag, it looks at the node's materials. A material that binds the
   parameter with the hash `0x88a0df95` clears the bits. That hash is
   `~crc32("screenSpaceRefractionTex")`, found by a search of the executable's own strings.

So the default is that a piece of the circuit casts a shadow onto a craft, and a refractive piece
does not. This agrees with the hull read: on Modesto's pole slot the top of the hull gets no sun,
and no craft is above it.

**Live: with the bits cleared, the hull's shade lifts (confidence 80).** On the port, on Modesto's
pole slot in a Time Trial, `scripts/wipeout-pc-replay-shadow-live.py` with `NOCAST=1` cleared the
two bits on every record of the table at `*0x020084d0`. The count is the `u16` at `+0x38` of the
model at `**0x02007fe0`, and the records are `0xe0` bytes. Modesto has 2,762 nodes and all 2,762
had the bits. The hull's `shadow` is `A / (A + B)` of the two knockout states of
[hull-tone.md](hull-tone.md), before and after, on five and four frames:

| Box (1280x720) | `shadow` as shipped (R, G) | `shadow` with the bits cleared (R, G) |
| --- | --- | --- |
| spine `(610, 430)-(670, 465)` | `0.01, 0.02` | `0.27, 0.27` |
| left fin `(460, 570)-(540, 630)` | `0.21, 0.13` | `0.53, 0.66` |
| housing bay `(590, 490)-(690, 570)` | `0.08, 0.07` | `0.16, 0.15` |
| track panel `(780, 390)-(880, 440)`, a control | `0.54, 0.54` | `0.50, 0.50` |

With the lighting as shipped, the left fin's mean goes from `(0.092, 0.111, 0.129)` to
`(0.175, 0.255, 0.242)`. So the game reads the bits in each frame, and the pieces that have them
shade the hull. The term does not reach 1. The values are after the tone map, and the fin is at
the top of the range in part. A second source of shade is not ruled out.

**What tests the bits: a pass for each model instance in `Scene_PrepareFrame` (confidence 65).**
In 1.07 `FUN_015f2d30`, inside the loop over the model instance records (`0x01edf3e0`, `0x360`
bytes each):

1. `FUN_01669e20(3, 0x020e3a80, i, 0)` at `0x015fb15e`. The read is that it selects target `i` of
   a set at `0x020e3a80`. The function was not read.
2. `FUN_0165a210(2, circuit model, 0x020e39d0 + 16 * i, record + 0xa0, ...)` at `0x015fb1d6`
   returns a count of circuit nodes. Their indices are the `u16` list at `0x0201ec00`. The third
   argument is a 16-byte entry for the instance and the fourth is the instance's matrix. The read
   is that this is a query for the nodes near the instance.
3. The mask is `0x4000` (`0x015fb1e3`), or `0x2000` (`0x015fb1eb`) if the byte at `*0x01ed4950` is
   not 0. That byte is the result of a compare of two floats of the lighting block
   (`0x01eb2a64 < 0x01eb2da4`), written earlier in the same function. The two floats are not
   named.
4. A node is drawn if its word has the mask bit, does not have bit `0x20`, and its type byte
   (`+0xb`) is 1. One draw is queued for each of the node's parts.
5. `FUN_016292d0(instance, queue, 2)` then queues the instance's own model.

So the map of one craft holds the circuit pieces near it and the craft, and both shade its hull.
This is the same shape as the 2048 read below. The matrices of the pass come from a block for
each instance at `0x020e3550`. Their law, the size of the map and its program were not read.

**The map of an instance, read live (confidence 80 for the values, 60 for the law).**
`scripts/wipeout-pc-replay-shadow-live.py` with `DUMP=1` stopped the port on Modesto's grid and
read the blocks that the pass uses. Two runs: a Time Trial on the pole slot, and a Single Race on
slot 8 with eight craft.

| Block | What it holds | Pole slot | Slot 8 |
| --- | --- | --- | --- |
| `0x020e3550 + 0x40 * i` | the view-projection of instance `i`, row vectors | one filled | eight filled |
| its third axis, normalised | the view direction | `(0.4016, -0.5098, 0.7608)` | the same |
| its first two column lengths | `1 /` the half-extents | `5.294` by `4.639` units | `5.028` by `4.775` units |
| its third column length | `1 /` the depth span | `377.0` units | `377.0` units |
| the world point at depth 1 on its axis | the far plane | `(-307.86, 195.29, -428.06)` | `(-288.19, 194.08, -286.48)` |
| `0x020e3790 + 0x40 * i` | the projection alone: `diag(sx, sy, -1 / 377)`, `tz = -378 / 377` | `sx 0.18889, sy 0.21556` | `sx 0.19889, sy 0.20943` |
| `0x020e39d0` | the 16-byte entry of the node query | `(-611.08, 580.20, -1002.45, 1)` | `(-591.41, 578.99, -860.87, 1)` |
| `0x020e3ab8` | a texture descriptor word, `0x707fc1ff` | width and height fields `0x1ff` | the same |

`Lighting.Sun direction` read `(-0.4016, 0.5098, -0.7608)` in both runs.

- **The map is orthographic and looks along the sun.** The view direction is the sun direction
  times `-1`, to four digits.
- **The far plane is at the craft and the map reaches 377 units toward the sun.** The near plane
  is at 1 and the far plane at 378 (`-1 / 377` and `-378 / 377`). On slot 8 the far-plane point is
  within 6 units of ours' spawn on that slot, `(-290.5, 196.7, -291.7)`.
- **The box is about 10 by 9.5 units and follows the craft.** The half-extents and the first two
  axes differ between the two poses, so they are fitted to the craft. The fit was not read.
- **The query's entry is the far-plane point plus 755 units of the sun direction**, in both runs.
  In the Single Race the entries after the first read 0. That nodes are queued for the first
  instance alone is not shown: the read is one stop of one frame.
- **The map is 512 by 512 (confidence 65).** The descriptor word has `0x1ff` in its low 14 bits
  and in the next 14. In the console's texture descriptor those are a width and a height less 1.
- **The mask is `0x2000` on Modesto.** The byte at `*0x01ed4950` read 1. The two floats of the
  compare read `-50.0` (`0x01eb2a64`) and `200.69` (`0x01eb2da4`). The second is equal to
  `shadowCoeffs`' `c0` in the same frame.

**The hull's lookup, read off its program (confidence 70).** `gcn_dump` of
`data/materials/ships/diffuse_with_specular_from_alpha_n_vcol.rcsmaterial` in `data08.psarc`, the
pixel program at `0x7c44`, the one that [hull-tone.md](hull-tone.md) reads:

- The coordinates come in as one interpolant, `attr0`. The program divides `x` and `y` by
  `max(0.01, w)` and adds `0.5` (`@1e8`-`@218`).
- It multiplies them by `shadowCoeffs[0]` and `[1]`, subtracts `0.5`, and takes the floor and the
  fraction (`@204`-`@240`). Those two constants are thus the size of the map in texels.
- It reads the map with four `image_gather4_c_lz_o` (`@2e4`, `@2ec`, `@304`, `@30c`): a compare of
  depth, four texels each, at four offsets. The sixteen results give nine taps, each `1 / 9`
  (`@3fc`).
- The depth that it compares has a bias. One part follows the slope of the surface: the program
  builds it from products of the coordinates with `shadowCoeffs[2]`, `[3]` and `[7]`, a reciprocal
  and `shadowCoeffs[8]` (`@1bc`-`@2b0`). The other part is the constant `shadowCoeffs[9]`
  (`@2b4`-`@2f8`).
- It does not read `shadowCoeffs[12]` to `[15]`, the row that the road's exponential term uses.
  So the road's `S = 1` knockout cannot release the hull, which is what the live read found.

The hull's `shadowCoeffs` cannot be the frame's block. That block's first row read
`(0.1309, 1.0, -0.0755, 1.0)` on Modesto, and a map size in texels is not `0.13`. The hull's draw
thus has a block of its own under the same name (confidence 60). Its values were not read.

**`track_proximity_shadow` is in the executable and nothing uses it (confidence 70).** The shadow
module's static initialiser `FUN_01670010` registers a depth-only pair by that name (`LEA` at
`0x01670239` and `0x016702bd`). The programs are at `0x019c97e0` and `0x019c96f0`. A scan of the
text segment for an instruction that addresses either descriptor, or any member of the module
above `0x020e51d8`, finds the initialiser alone. The names are not compared against anywhere else.
The pair is thus not the caster program on Omega.

**2048: checked, differs in what is read.** 2048 ships the pair by the same names
([`docs/formats/gxp.md`](../../../formats/gxp.md)). In the 2048 read, track pieces beside the
craft are drawn from the sun into a map of the craft's own
([`docs/rendering/shadows.md`](../../../rendering/shadows.md), confidence 62 there). Whether 2048's
loader has the two tags was not checked: no 2048 executable was on the machine that made this read.

## Wired: Omega's hull takes the shade of the circuit (2026-11-27, lane `hull-shadow-wire`)

Ours now draws the per-craft map that the section above reads. The hull compares against it with nine taps. The
work is in `Title::hull_shade`, `HullShade::CircuitAndSelf { reach: 377.0 }` on Omega, and in `oag_render::shadow`.

**What the lane built.**

- The map is orthographic along `Lighting.Sun direction`. `Fit::matrix_spanning` gives it a depth that runs unequally
  either side of the craft. The map reaches 377 units toward the sun from the craft. Ours' far plane is 7 units past
  the craft, so the depth is 384 units. That is chosen, not measured. The original's far plane is at the craft, and its
  exact offset is unread.
- The box is `CIRCUIT_SHADE_HALF_EXTENT = 7.0` units each way across the sun. This is chosen, not measured. The
  original's box has half-extents of 5.29 by 4.64 on the pole slot and 5.03 by 4.78 on slot 8. The fit rule is unread.
- The casters are the circuit's `draws` and `cutouts` in the craft's light column, then the craft. The circuit casts
  with no face culling. That is chosen, not measured.
- The craft also casts with no face culling on this path. That is chosen, not measured. It is the variant that agrees
  with the original on five slot 8 boxes ("The craft's own faces in the map" below).
- Transparent circuit draws do not cast. This stands in for the refractive-material rule. It is chosen, not measured.
  Cutouts cast as solid. That is also chosen, not measured.
- The hull reads the depth map alone, with no road mask. It takes nine compared taps, three by three, a texel apart.
  `CIRCUIT_SHADE_BIAS = 0.0002` of the map's depth is the bias. It is chosen, not measured. The original's bias terms are
  in `shadowCoeffs`, and those values are not read.
- It applies under `--shadows original` only. The `mapped` tier keeps the road mask and the craft. Every other title
  keeps the earlier path.
- The craft's title gives the `hull_shade`.
- `OAG_DEBUG_HULL_SHADE_STATE=a` or `b` is the measuring switch. It renders the two knockout states of
  [hull-tone.md](hull-tone.md), "The shadow term, read by knockout". It also turns the hull's engine and detonator lights
  off, as the original's run had the engine light off.

**Measured, ours against the original.** Ours is the branch's release build, stripped, on a Steam Deck (RADV), headless.
It ran `--shadows original`, 1280x720, Modesto Heights, `--team FEISAR`, ticks 600 and 603, mean of 2 frames. The
original's frames are the read lane's port frames. Slot 8 has 5 frames for each state. The pole slot has 9. The boxes are the same on both sides, because the poses agree. Values are sRGB-decoded means after the tone
map. The ratio is `A / (A + B)` as in [hull-tone.md](hull-tone.md), as (R, G, B).

| Slot | Box (x0,y0,x1,y1) | Original A | Original B | Original ratio | Ours A | Ours B | Ours ratio |
| --- | --- | --- | --- | --- | --- | --- | --- |
| pole | housing 590,470,690,560 | .012 .012 .036 | .172 .188 .193 | .06 .06 .16 | 0 0 0 | .146 .159 .172 | 0 0 0 |
| pole | spine 610,410,670,450 | .004 .006 .010 | .431 .493 .195 | .01 .01 .05 | 0 0 0 | .426 .479 .182 | 0 0 0 |
| pole | nose 620,360,660,400 | .040 .054 .071 | .583 .633 .670 | .06 .08 .10 | 0 0 0 | .563 .620 .680 | 0 0 0 |
| pole | left wing 470,580,540,620 | .039 .041 .044 | .121 .349 .609 | .25 .10 .07 | 0 0 0 | .086 .296 .606 | 0 0 0 |
| pole | right wing 760,580,830,620 | .018 .022 .023 | .104 .370 .619 | .14 .06 .04 | 0 0 0 | .104 .363 .649 | 0 0 0 |
| 8 | housing | .013 .018 .037 | .118 .134 .146 | .10 .12 .20 | .040 .040 .024 | .135 .149 .140 | .23 .21 .15 |
| 8 | spine | .212 .246 .130 | .218 .197 .052 | .49 .56 .71 | .414 .465 .166 | .065 .059 .012 | .86 .89 .93 |
| 8 | nose | .608 .625 .632 | .348 .347 .293 | .64 .64 .68 | .618 .654 .671 | 0 0 0 | 1 1 1 |
| 8 | left wing | .216 .438 .589 | .211 .275 .261 | .51 .61 .69 | .091 .347 .589 | 0 0 0 | 1 1 1 |
| 8 | right wing | .075 .139 .215 | .103 .243 .400 | .42 .36 .35 | .038 .102 .165 | .076 .282 .464 | .34 .27 .26 |
| 8 | track control 340,360,440,410 | .100 .113 .108 | .103 .117 .113 | .49 .49 .49 | .109 .122 .119 | .116 .130 .128 | .48 .48 .48 |

**Reading.**

- **Pole slot: the acceptance holds.** The spine reads 0.00 against the original's 0.01. The housing
  reads 0.00 against 0.06 to 0.16, under the 0.2 limit. Ours' B agrees with the original's B within about 5 % on the
  spine, the nose and the right wing. On the wings the original's ratio is 0.04 to 0.25 and ours is 0. The wings were
  not part of the acceptance.
- **Slot 8, where the shade falls.** Both sides have the dark housing bay and the part-shaded right wing.
- **Slot 8, sunlit panels: the ratio differs (confidence 60 for the cause).** Ours reads 0.86 to 1.0. The original reads
  0.49 to 0.64 (R, G), and its blue reaches 0.71. The original has an additive term in both states that ours has not. On the pole slot its A is neutral grey
  (`(.039, .041, .044)` on the blue left wing) where its B is blue. On slot 8 its B on the sunlit left wing is near
  neutral (`(.211, .275, .261)`) where its A is blue. A term that does not follow the albedo and is in both states is not
  the shadow. With it subtracted, the original's shadow on the sunlit left wing is about 0.8 to 1.0. The cause is not
  proven. The hull's reflection is the candidate. Ours does not build it.
- **Slot 8 spine: acceptance not met.** "The term that is not shadow, read directly" below has the later read.
  It supersedes the numbers in this bullet for the original.
  Ours reads 0.86, 0.89 and 0.93. The acceptance was within 0.1 of 0.36 to 0.42.
  Two reads of the original on the box `(610, 410)-(670, 450)` disagree. [hull-tone.md](hull-tone.md) has 0.36, 0.42
  and 0.71. This run's five-frame mean has 0.49, 0.56 and 0.71. Frame by frame, the original's box moves a lot on that
  panel. Over two runs of the same route, A goes from 0.17 to 0.25 and B from 0.19 to 0.36 (red). Thus the spine box on
  slot 8 is not a stable acceptance box.
- **Slot 8 housing on the doc's box.** Ours reads 0.21 to 0.23 (R, G) against the original's 0.10 to
  0.12. That is over the 0.2 limit by a few hundredths. On a tighter box, `(620, 470, 660, 530)`, ours reads 0.11 to 0.13.
  There is no reading of the original on that tighter box.
- **Bias.** `CIRCUIT_SHADE_BIAS` stays at 0.0002. The nose and the left wing show no acne, because their B is 0. The
  spine's B is 0.065. The bay reads as shaded.
- **Control.** The track panel reads 0.48 on ours and 0.49 on the original. So A and B are almost equal on that box on
  both sides. From this we infer that the two states have the same exposure. That is an inference, not a measurement.

### The term that is not shadow, read directly (2026-11-27)

The reading above infers a term that is in both states. This run reads it.
`scripts/wipeout-pc-replay-shadow-live.py` has a fourth state for it, `NEITHER=1`. The state D has
`Sun color` at 0 and both `K` and `Ks` at 0. What the hull shows in D is in A and in B alike. The
table gives `(A - D) / ((A - D) + (B - D))`, the ratio with that term subtracted.

The run is on the port on the Steam Deck, with the same two routes and the same boxes. Slot 8 has
three frames for each of A, B and D. The pole slot has four. A state's frames are the port's
screenshots from the state's `from` time to two seconds before its `to` time. The screenshot at the
`to` time already shows the next state.

| Slot | Box | Original D | Original, D subtracted (R, G, B) | Ours (R, G, B) |
| --- | --- | --- | --- | --- |
| 8 | nose | `.326 .323 .271` | `1.00, 0.99, 0.98` | `1, 1, 1` |
| 8 | left wing | `.168 .242 .229` | `0.83, 0.97, 0.97` | `1, 1, 1` |
| 8 | right wing | `.045 .063 .088` | `0.41, 0.38, 0.37` | `0.34, 0.27, 0.26` |
| 8 | housing | `.007 .011 .029` | `0.08, 0.10, 0.14` | `0.23, 0.21, 0.15` |
| 8 | spine | `.044 .048 .042` | `0.47, 0.54, 0.92` | `0.86, 0.89, 0.93` |
| pole | nose | `.038 .053 .070` | `0.01, 0.00, 0.00` | `0, 0, 0` |
| pole | spine | `.004 .006 .010` | `0.00, 0.00, -0.02` | `0, 0, 0` |
| pole | housing | `.011 .013 .035` | `0.02, 0.01, 0.05` | `0, 0, 0` |
| pole | right wing | `.014 .018 .019` | `0.02, 0.02, 0.01` | `0, 0, 0` |

- **The term is real, and it is large on the sunlit nose (confidence 85).** On slot 8 the nose reads
  `(.326, .323, .271)` in D. Its B is `(.325, .326, .277)`, so B less D is zero there. The nose's
  `shadow` is thus 1.0 on the original, as on ours. The raw ratio of 0.64 was this term and not shade.
- **The sunlit panels agree with it subtracted.** The nose and the left wing read 0.83 to 1.00 on the
  original and 1 on ours.
- **The part-shaded right wing is near.** The original reads 0.37 to 0.41 and ours 0.26 to 0.34.
- **The pole slot agrees on every box.** The original reads 0.00 to 0.05 and ours 0.
- **Two boxes still differ, both at the housing bay (confidence 70).** The housing reads 0.08 to 0.10 on
  the original and 0.21 to 0.23 on ours. The spine box is at the top edge of the bay. It reads 0.47 to
  0.54 on the original (R, G). Ours reads 0.86 to 0.89. So the original's shade in and above the bay
  covers more than ours'. The cause is not read. One candidate is the craft's own faces in the map.
  Ours draws the craft with its front faces culled. The original's caster program is unread.
  "The craft's own faces in the map" below resolves these two boxes.
- **What the term is.** "The leftover term is the hull's sun and a neutral term" below reads it. A write to
  `Sun color` does not reach the hull, so D still holds the hull's sun term.

The pole slot's left wing reads `0.31, 0.17, 0.11` on the original, from a grey A of
`(.088, .090, .093)`. One of its four A frames carries a bright patch there. It is not in the table.

**Not checked.** 2048: not checkable: no 2048 package on the machine. HD and 2048 stay pixel-identical under `original`
by construction, because their `hull_shade` keeps the earlier path. That was not measured: no HD image and no 2048
package were on the machine.

### The leftover term is the hull's sun and a neutral term (2026-11-27)

State D has `Sun color` at 0. That write does not reach the hull. The hull's sun term reads the live
triple at `0x800eb4810` ([hull-tone.md](hull-tone.md), "The measurement"). So D still holds the hull's
sun term. This run reads the two parts of D.

`LIVESUN=1` adds two states on top of D. State E also holds the live triple at 0. The triple reads
`(4, 3.608, 2.165)` before the write. State F also holds the three floats at `0x800eb4800` at 0. They
read `(0.310, 0.259, 0.714)`, the settings object's ambient. Slot 8 has four frames for each state. The pole
slot has four to six.

| Slot | Box | D | E, live sun at 0 | F, ambient at 0 too | D less E |
| --- | --- | --- | --- | --- | --- |
| 8 | nose | `.318 .317 .261` | `.138 .160 .187` | `.146 .169 .196` | `.180 .156 .074` |
| 8 | left panel `(555, 420)-(600, 450)` | `.526 .588 .525` | `.351 .360 .373` | `.344 .353 .368` | `.176 .229 .152` |
| 8 | left wing | `.147 .212 .209` | `.103 .114 .130` | `.121 .132 .146` | `.044 .097 .079` |
| 8 | right wing | `.046 .063 .090` | `.040 .054 .085` | `.041 .055 .086` | `.005 .009 .005` |
| 8 | housing | `.006 .007 .018` | `.013 .015 .039` | `.010 .013 .034` | `-.007 -.008 -.021` |
| 8 | track control | `.107 .122 .120` | `.049 .069 .112` | `.050 .071 .115` | `.058 .052 .008` |
| pole | nose | `.038 .053 .070` | `.052 .069 .097` | `.045 .062 .087` | `-.014 -.016 -.028` |
| pole | left panel | `.138 .161 .189` | `.104 .138 .202` | `.088 .118 .175` | `.034 .023 -.013` |
| pole | left wing | `.023 .026 .028` | `.018 .019 .024` | `.024 .026 .031` | `.006 .007 .004` |
| pole | track control | `.344 .083 .094` | `.437 .121 .136` | `.439 .121 .136` | `-.094 -.038 -.042` |

- **Part of D is the hull's sun term (confidence 80).** On slot 8 the nose loses `(.180, .156, .074)` when the
  live triple goes to 0. The loss is warm, as the sun is. On the pole slot, in the shade, the nose does not
  lose light. The right wing on slot 8, which is in shade, loses nothing.
- **The rest is a neutral term that needs no sun (confidence 75).** With the live sun at 0 the slot 8 nose still
  reads `(.138, .160, .187)` and the left panel `(.351, .360, .373)`. The term is near neutral on a yellow panel
  and on a blue wing. It does not follow the albedo. It is in the shade too: the pole slot's left panel
  reads `(.104, .138, .202)`.
- **The ambient at `0x800eb4800` does not move the hull.** F agrees with E within the frame noise. This agrees
  with [hull-tone.md](hull-tone.md).
- **The reflection is the candidate for the neutral term.** The hull program adds
  `albedo.a * (6.87 * (1 - n.v)^9.23 + Chromeness + 0.1) * cubeReflectionTex` after the albedo product, and it
  does not multiply that term by `shadow` ([hull-tone.md](hull-tone.md)). What the cube holds is unread.
- **Limits.** The picture's exposure moves when the sun goes to 0. The pole slot's track control box goes from
  `.344` to `.437` in red. So E and F are not on D's exposure, and the neutral term's size has that error. The
  original's frames also move a lot from frame to frame: the slot 8 nose reads `.254` to `.403` over D's four frames.
- **Ours.** Ours has no neutral term on Omega's hull. Ours' sunlit nose reads `(.317, .296, .215)` as shipped
  against the original's `(.335, .337, .298)`. Ours gets there with sun and ambient alone, so ours' sun term on
  the hull is larger than the original's. That agrees with the sun ratio in [hull-tone.md](hull-tone.md).

### The craft's own faces in the map (2026-11-27)

This run tests the candidate above. Ours draws the craft into the circuit-shade map in three ways.

The run is ours on the Steam Deck (RADV), release build, Modesto Heights, 1280x720, tick 600,
`--shadows original`. The value is `shadow = A / (A + B)` on the two knockout states, as (R, G, B). The
original's column is the one above, with its term D subtracted. The run used a temporary switch, which is
not in the tree.

| Slot 8 box | Original | Ours, craft front faces culled | Ours, craft not culled |
| --- | --- | --- | --- |
| housing (590,470,690,560) | 0.08, 0.10, 0.14 | 0.23, 0.22, 0.15 | 0.06, 0.06, 0.07 |
| spine (610,410,670,450) | 0.47, 0.54, 0.92 | 0.87, 0.89, 0.93 | 0.53, 0.61, 0.89 |
| nose (620,360,660,400) | 1.00, 0.99, 0.98 | 1, 1, 1 | 0.99, 0.98, 0.98 |
| left wing (470,580,540,620) | 0.83, 0.97, 0.97 | 1, 1, 1 | 0.96, 0.99, 0.99 |
| right wing (760,580,830,620) | 0.41, 0.38, 0.37 | 0.34, 0.27, 0.26 | 0.34, 0.27, 0.26 |

The pole slot reads 0 on every box in all three ways. The third way culls the back faces. It reads the
same as "not culled" within 0.01.

- **With every face of the craft in the map, all five slot 8 boxes agree with the original.** The
  confidence that the original's craft casts with its lit faces is 75. The housing, the spine and the
  nose are within 0.07 on each channel. The wings are within 0.13. With front faces culled, the housing
  and the spine are 0.12 to 0.40 away (R, G).
- **The right wing does not change.** Both ways read 0.34, 0.27 and 0.26 there. Its remaining difference
  is thus not from the craft's faces.
- **Why the faces change the bay.** With its lit faces in the map, the hull's lit faces compare against
  themselves through the receiver's bias. The walls of the housing bay then shade the bay.
- **Ours now draws the craft with no face culling on this path.** The original's caster program is unread,
  so this rule is chosen, not measured. Back-face culling reads the same within 0.01, and no measurement here
  separates the two.

## Open

- The pass for each model instance at `0x015fb15e`: the fit of its box and its first two axes to the craft, the query
  `FUN_0165a210`, the program of the map, and the bind that gives it to the hull's `shadowMapSampler`.
- Whether circuit nodes are queued for every instance or for the first alone.
- The values of the hull draw's own `shadowCoeffs` (the size of the map, the three bias terms), and the vertex program's
  matrix for `attr0`.
- The two floats that choose between bits `0x2000` and `0x4000` (`0x01eb2a64`, `0x01eb2da4`).

- Which models set bit 1 of `+0x110`, and what the second caster list (`*0x020e4268`) holds.
- The `pcf` taps and the cascade selection inside the receiver (`@0xa764`'s `@0x7b8`-`@0x964` per
  [panel-displacement.md](panel-displacement.md)).
- Placing `FUN_015f2d30`, `FUN_01670960`, `FUN_01670ba0` and `FUN_01670930` in the base eboot, after which they can take
  `names.tsv` rows (`Scene_PrepareFrame` is already mapped: base `0x015c63a0`).
