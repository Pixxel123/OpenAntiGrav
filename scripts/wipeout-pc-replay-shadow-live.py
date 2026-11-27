#!/usr/bin/env python3
"""Live read of the hull's shadow term on the `wipeout_pc` native port (1.07 eboot), walked by the port's own pad replay.

    python3 wipeout-pc-replay-shadow-live.py <extracted port build dir> <route.pad> <out dir>

For a machine with no headless compositor and no virtual pad (a Steam Deck): the route (`GOW3_PAD_REPLAY`) walks to a grid
and ends on the START RACE press, and the port's own screenshots (`GOW3_SCREENSHOT_EVERY=1`) are the frames. This process
starts the port, so `/proc/<pid>/mem` is open to it. It switches the engine light off, finds the lighting object by Modesto
Heights' authored floats, and holds three states with the sun colour at 0: `K 4, Ks 0`, then `K 0, Ks 4`, then the sun
alone. `KNOCK=1` repeats the first two with the shadow map forced to 1, and `HOLD` is the seconds of frames per state.
`DUMP=1` adds the raw blocks of the pass for each model instance to `note.json`.
`NEITHER=1` adds a state with the sun and both ambient scales at 0.
`LIVESUN=1` adds two more on top of it: the hull's live sun triple at 0, then the specular's too.
`NOCAST=1` then clears the circuit nodes' caster bits and repeats the two again. `note.json` has the wall-clock span of each state; `wipeout-pc-replay-shadow-ratio.py` reads the frames.
`SETTLE` is the seconds from the last replayed press to the first state. Every write is to this run's own process.
`docs/ghidra/functions/ps4-omega-eu/hull-tone.md`, "The shadow term, read by knockout".
"""
import json, os, signal, struct, subprocess, sys, threading, time
from pathlib import Path

HOME = Path.home(); DIAG = HOME / "diag"
BUILD = sys.argv[1]; ROUTE = sys.argv[2]; OUT = Path(sys.argv[3]); OUT.mkdir(parents=True, exist_ok=True)
BASE, GHIDRA = 0x800000000, 0x01000000
PRODUCER = 0x0173E350
PROLOGUE = bytes.fromhex("554889e5415741564154534883ec204989fc498bbc241801")
KEY = struct.pack("<4f", 0.0, 1.0, 2.264, 0.25)
note = {"states": []}

def rt(a): return BASE + a - GHIDRA

def regions(pid):
    out = []
    for line in open("/proc/%d/maps" % pid):
        p = line.split(); lo, hi = (int(x, 16) for x in p[0].split("-"))
        name = p[5] if len(p) > 5 else ""
        if "r" in p[1] and "w" in p[1] and (not name or name == "[heap]") and hi - lo <= (1 << 30):
            out.append((lo, hi))
    return out

def scan(fd, pid, pat):
    hits = []
    for lo, hi in regions(pid):
        pos = lo
        while pos < hi:
            n = min(64 << 20, hi - pos)
            try: blob = os.pread(fd, n, pos)
            except OSError: break
            i = blob.find(pat)
            while i != -1:
                hits.append(pos + i); i = blob.find(pat, i + 1)
            pos += n
    return hits

def game_pid():
    for d in os.listdir("/proc"):
        if d.isdigit():
            try:
                if open("/proc/%s/comm" % d).read().strip().startswith("gow3-probe"): return int(d)
            except OSError: pass
    return None

(DIAG / "pad.ctl").write_text("replay\n")
env = dict(os.environ, GOW3_PAD_FILE=str(DIAG / "pad.ctl"), GOW3_PAD_REPLAY=ROUTE, GOW3_SCREENSHOT_EVERY="1", PYTHONUNBUFFERED="1")
log = open(OUT / "game.log", "w")
proc = subprocess.Popen(["./AppRun", "--play"], cwd=BUILD, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, start_new_session=True)
finished = threading.Event()
def pump():
    for line in proc.stdout:
        log.write("%s %s" % (time.strftime("%H:%M:%S"), line)); log.flush()
        if "pad replay finished" in line: finished.set()
threading.Thread(target=pump, daemon=True).start()
try:
    if not finished.wait(240): sys.exit("STALLED: the replay did not finish")
    time.sleep(float(os.environ.get("SETTLE", "9")))
    pid = game_pid()
    if pid is None: sys.exit("STALLED: no gow3-probe")
    fd = os.open("/proc/%d/mem" % pid, os.O_RDWR)
    got = os.pread(fd, len(PROLOGUE), rt(PRODUCER))
    note["producer_ok"] = got == PROLOGUE
    if not note["producer_ok"]: sys.exit("STALLED: producer bytes differ: %s" % got.hex())
    os.pwrite(fd, b"\xc3", rt(PRODUCER))
    hits = scan(fd, pid, KEY)
    note["key_hits"] = [hex(h) for h in hits]
    image = [h for h in hits if h >= BASE]
    if not image: sys.exit("STALLED: lighting object not found: %s" % note["key_hits"])
    base = image[0]
    note["base"] = hex(base)
    note["block"] = list(struct.unpack("<16f", os.pread(fd, 64, base - 24)))
    # The shadow module's caster inputs, read with the game stopped
    # (`docs/ghidra/functions/ps4-omega-eu/track-shadow.md`, "Casters").
    # The frame clears and refills the list, so the stop is retried until it lands on a filled one.
    try:
        for attempt in range(200):
            os.kill(pid, signal.SIGSTOP)
            n = struct.unpack("<I", os.pread(fd, 4, rt(0x01EDC810)))[0]
            if 0 < n <= 4096:
                break
            os.kill(pid, signal.SIGCONT)
            time.sleep(0.0137)
        note["model_read_attempts"] = attempt + 1
        idx = struct.unpack("<%dI" % min(n, 512), os.pread(fd, 4 * min(n, 512), rt(0x01EDB240)))
        rows = []
        for i in idx:
            rec = os.pread(fd, 0x360, rt(0x01EDF3E0 + i * 0x360))
            rows.append({"i": i, "sphere": [round(x, 3) for x in struct.unpack_from("<4f", rec, 0xF0)], "flag110": rec[0x110]})
        note["models"] = {"count": n, "casters": [r for r in rows if r["flag110"] & 2], "not_casters": len([r for r in rows if not r["flag110"] & 2])}
        note["second_gate"] = struct.unpack("<I", os.pread(fd, 4, rt(0x020E4340)))[0]
        bank = struct.unpack("<Q", os.pread(fd, 8, rt(0x0204AB58)))[0]
        ptr = struct.unpack("<Q", os.pread(fd, 8, bank + 0x1050))[0]
        note["shadowCoeffs"] = [round(x, 4) for x in struct.unpack("<16f", os.pread(fd, 64, ptr))]
        note["sun_dir"] = [round(x, 4) for x in struct.unpack("<4f", os.pread(fd, 16, rt(0x01EB47D0)))]
        if os.environ.get("DUMP") == "1":
            # The blocks of the pass for each model instance, raw, for a read of its matrices
            # (`docs/ghidra/functions/ps4-omega-eu/track-shadow.md`, "What tests the bits").
            note["dump"] = {"instances_0x020e3540": os.pread(fd, 0x600, rt(0x020E3540)).hex(),
                            "records": {str(i): os.pread(fd, 0x360, rt(0x01EDF3E0 + i * 0x360)).hex() for i in idx},
                            "lighting_0x01eb2a40": os.pread(fd, 0x40, rt(0x01EB2A40)).hex(),
                            "lighting_0x01eb2d90": os.pread(fd, 0x20, rt(0x01EB2D90)).hex(),
                            "light_view_0x0221af60": os.pread(fd, 0x40, rt(0x0221AF60)).hex(),
                            "camera_view_0x0221afa0": os.pread(fd, 0x40, rt(0x0221AFA0)).hex(),
                            "mask_flag": os.pread(fd, 1, struct.unpack("<Q", os.pread(fd, 8, rt(0x01ED4950)))[0]).hex()}
    except (OSError, struct.error) as e:
        note["caster_read_error"] = str(e)
    finally:
        os.kill(pid, signal.SIGCONT)
    def put(off, vals): os.pwrite(fd, struct.pack("<%df" % len(vals), *vals), base + off)
    keep = os.pread(fd, 64, base - 24)
    def state(tag, writes, settle, hold):
        os.pwrite(fd, keep, base - 24)
        for off, vals in writes: put(off, vals)
        time.sleep(settle)
        t0 = time.time(); time.sleep(hold); t1 = time.time()
        note["states"].append({"tag": tag, "from": time.strftime("%H%M%S", time.localtime(t0)), "to": time.strftime("%H%M%S", time.localtime(t1))})
        print(tag, note["states"][-1], flush=True)
    # `shadowCoeffs`' fourth row is scaled by these two literals; both at 0 makes `S = 1` on every receiver
    # (`docs/ghidra/functions/ps4-omega-eu/track-shadow.md`, "The knockout").
    knock, knock_keep = rt(0x018632C0), struct.pack("<2f", -80.0, -160.0)
    note["knock_ok"] = os.pread(fd, 8, knock) == knock_keep
    if not note["knock_ok"]: sys.exit("STALLED: the shadow literals are not at the mapped address")
    hold = float(os.environ.get("HOLD", "5"))
    sun0 = (-24, [0.0, 0.0, 0.0])
    state("s0-baseline", [], 0.5, 2)
    state("sA-sun0-K4-Ks0", [sun0, (12, [4.0]), (16, [0.0])], 2, hold)
    state("sB-sun0-K0-Ks4", [sun0, (12, [0.0]), (16, [4.0])], 2, hold)
    state("sC-sun0-only", [sun0], 1.5, 2)
    if os.environ.get("NEITHER") == "1":
        # What the hull still shows with the sun and both ambient scales at 0 is in A and in B alike,
        # so it is subtracted before the ratio
        # (`docs/ghidra/functions/ps4-omega-eu/track-shadow.md`, "Wired").
        state("sD-sun0-K0-Ks0", [sun0, (12, [0.0]), (16, [0.0])], 2, hold)
    if os.environ.get("LIVESUN") == "1":
        # A write to `Sun color` does not reach the hull: its sun term reads the live triple at
        # `0x800eb4810`, and the four floats before it are the specular's
        # (`docs/ghidra/functions/ps4-omega-eu/hull-tone.md`, "The measurement"). These two
        # states zero them in turn on top of D, and hold the write against a refill.
        live, spec = 0x800EB4810, 0x800EB4800
        live_keep, spec_keep = os.pread(fd, 12, live), os.pread(fd, 12, spec)
        note["live_sun"] = list(struct.unpack("<3f", live_keep)); note["live_spec"] = list(struct.unpack("<3f", spec_keep))
        def held(tag, addrs, settle, hold):
            os.pwrite(fd, keep, base - 24)
            for off, vals in [sun0, (12, [0.0]), (16, [0.0])]: put(off, vals)
            t = time.time(); t0 = t + settle
            while time.time() < t0 + hold:
                for addr in addrs: os.pwrite(fd, bytes(12), addr)
                time.sleep(0.02)
            note["states"].append({"tag": tag, "from": time.strftime("%H%M%S", time.localtime(t0)), "to": time.strftime("%H%M%S", time.localtime(time.time()))})
            print(tag, note["states"][-1], flush=True)
        held("sE-D-and-live-sun0", [live], 2, hold)
        held("sF-D-and-live-sun0-spec0", [live, spec], 2, hold)
        os.pwrite(fd, live_keep, live); os.pwrite(fd, spec_keep, spec)
    if os.environ.get("KNOCK") == "1":
        os.pwrite(fd, bytes(8), knock)
        state("s1-map1", [], 2, 3)
        state("sA1-map1-sun0-K4-Ks0", [sun0, (12, [4.0]), (16, [0.0])], 2, hold)
        state("sB1-map1-sun0-K0-Ks4", [sun0, (12, [0.0]), (16, [4.0])], 2, hold)
        os.pwrite(fd, knock_keep, knock)
    if os.environ.get("NOCAST") == "1":
        # The circuit loader marks each node as a caster onto craft in bits 0x6000 of `+0xa0` of its 0xe0-byte record
        # (`docs/ghidra/functions/ps4-omega-eu/track-shadow.md`, "The circuit's pieces cast onto craft"). Clear them all.
        table = struct.unpack("<Q", os.pread(fd, 8, rt(0x020084D0)))[0]
        model = struct.unpack("<Q", os.pread(fd, 8, struct.unpack("<Q", os.pread(fd, 8, rt(0x02007FE0)))[0]))[0]
        count = struct.unpack("<H", os.pread(fd, 2, model + 0x38))[0]
        marked = 0
        for i in range(count):
            at = table + i * 0xE0 + 0xA0
            word = struct.unpack("<I", os.pread(fd, 4, at))[0]
            marked += bool(word & 0x6000)
            os.pwrite(fd, struct.pack("<I", word & 0xFFFF9FFF), at)
        note["nocast"] = {"table": hex(table), "nodes": count, "marked": marked}
        print("nocast", note["nocast"], flush=True)
        state("s2-nocast", [], 2, 3)
        state("sA2-nocast-sun0-K4-Ks0", [sun0, (12, [4.0]), (16, [0.0])], 2, hold)
        state("sB2-nocast-sun0-K0-Ks4", [sun0, (12, [0.0]), (16, [4.0])], 2, hold)
    os.pwrite(fd, keep, base - 24)
finally:
    (OUT / "note.json").write_text(json.dumps(note, indent=1))
    try: os.killpg(proc.pid, signal.SIGTERM)
    except OSError: pass
    time.sleep(3)
    subprocess.run(["pkill", "-x", "gow3-probe"])
print("done", OUT / "note.json")
