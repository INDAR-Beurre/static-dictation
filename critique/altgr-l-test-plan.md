# AltGr+L — end-to-end trace, risk table, test plan

Date: 2026-11-15 · Host: Wayland (`wayland-0`), keyd v2.6.0 active, 22 input nodes armed.

**Verdict: AltGr+L works as-is = YES (conditional).** Verified live by injecting a real chord into
the running binary: `AltGr↓ L↓ L↑ AltGr↑` → `hk: event20 -> Toggle` +
`static-dictation: toggle event received`. The re-emit path was read back and shows AltGr
forwarded to apps with L swallowed.

Conditions (all currently satisfied on this host):
1. Some node advertises **both** KEY_L (38) and KEY_RIGHTALT (100) — verified on `event1`
   (Dell receiver), `event9` (Surface kbd), `event20` (keyd vkbd).
2. `/dev/uinput` is writable so the node gets grabbed (→ L is swallowed).
3. No other process holds `EVIOCGRAB` on that node — probed: keyd holds none.
4. AltGr must be pressed **before** L, and AltGr must emit kernel code **100**.
5. The running binary must match `src/hk.rs` (binary here was newer than source; `cargo build`
   is a no-op 0.4 s — rebuild anyway when testing a source edit).

---

## (a) Findings — exact code path

### Detection (src/hk.rs)

| Step | Location | Behaviour |
|---|---|---|
| key codes | `src/hk.rs:17` `const KEY_L: u16 = 38;` · `src/hk.rs:20` `const KEY_RIGHTALT: u16 = 100;` | kernel evdev codes (not X keycodes) |
| log filter | `src/hk.rs:163-166` `fn tracked()` | matches L / KP0 / RIGHTCTRL / RIGHTALT |
| decide() | `src/hk.rs:214-258` | the whole state machine |
| idle-refresh | `src/hk.rs:224` `state.last_event = Instant::now();` | runs for **every** event, incl. repeats |
| AltGr down | `src/hk.rs:251-254` `KEY_RIGHTALT => { state.modifier_held = true; None }` | arms the chord |
| L down, chord | `src/hk.rs:246-250` `KEY_L if state.modifier_held && state.past(&state.last_toggle)` | fires `HotkeyEvent::Toggle`, clears `modifier_held` |
| L down, no chord | same arm, guard false → `None` | plain L flows through |
| AltGr up | `src/hk.rs:230-233` | clears `modifier_held` |
| stale-hold safety | `src/hk.rs:219-221`, `MODIFIER_IDLE_RESET` `src/hk.rs:26` (2 s) | clears a hold whose keyup was lost |
| debounce | `DEBOUNCE` `src/hk.rs:24` (175 ms) via `past()` `src/hk.rs:205-210` | re-tap inside 175 ms swallowed |
| value 2 (repeat) | `src/hk.rs:257` `_ => None` | ignored (but `last_event` already refreshed at :224) |

### The stale-hold safety is *correct* — and this refutes an earlier worry

An apparent bug ("hold AltGr ≥2 s then tap L silently fails") does **not** reproduce. Measured
live: AltGr holds of 1.0 / 2.5 / 5.0 s each produced `-> Toggle` normally. Reason:

- A physically held AltGr produces kernel **autorepeat** events (`value 2`). Verified on the
  wire: holding AltGr on `event1` for 1.5 s emitted `(1, …)` then ~40 × `value 2` then `(0)`.
- `value 2` still reaches `src/hk.rs:224`, so `last_event` stays fresh while the key is held →
  the 2 s stale reset never fires mid-hold.
- If the keyup is **lost** (`modifier_held` stuck), autorepeat has also stopped, so `last_event`
  ages out ≥2 s → the next L clears the stale hold **before** matching → no false toggle.
  (Reproduced: AltGr↓ with keyup withheld, 3 s idle, then L → exactly 1 toggle, the intended one.)

So the modifier logic is sound. No fix required.

### Re-emit path (grabbing does not break AltGr in apps)

| Step | Location | Behaviour |
|---|---|---|
| shared virtual kbd | `src/hk.rs:60-129` `UinputKbd`, created once at `src/hk.rs:490-492` | name `static-dictation-kbd`, `UI_SET_KEYBIT` for codes 1..=248 |
| grab decision | `src/hk.rs:297-308` | grab only if uinput exists **and** `has_altgr_l(fd)` |
| `has_altgr_l` | `src/hk.rs:194-202` (`EVIOCGBIT_KEY_64` = 0x8040_4521) | device must advertise bit 38 **and** bit 100 |
| grab | `src/hk.rs:304` `EVIOCGRAB` | non-zero (EBUSY) → silent read-only fallback |
| swallow | `src/hk.rs:343-357` | grabbed + `code == KEY_L` + the press produced `Toggle` → L press/repeat/release **not** re-emitted; AltGr (100) **is** |
| emit | `src/hk.rs:359-361` → `UinputKbd::emit` `src/hk.rs:115-121` | raw 24-byte event + one `EV_SYN` |
| feedback loop | `SELF_DEVICE_PREFIXES` `src/hk.rs:44` + `is_self_device` `src/hk.rs:183-191`, checked `src/hk.rs:293` | own vkbd and `burst-inject` never watched → no loop. Verified: `grep -c 'watching /dev/input/event22'` = 0 |
| rescan | `src/hk.rs:400-427`, 2 s poll | hot-plug re-arm |

**Does AltGr+L still reach foreground apps?** Yes. Reading our `static-dictation-kbd` node while
injecting into the grabbed `event20`, the re-emitted stream for the chord was `(100,1),(100,0)` —
AltGr passes through, L is swallowed. Apps see a bare AltGr press (no special char) — the intended
"consume the hotkey" semantics. Every *other* AltGr combination is untouched (plain L is
re-emitted, verified `(38,1),(38,0)`).

### Toggle handling (src/main.rs)

`HotkeyEvent::Toggle =>` `src/main.rs:119-138`: `recording=false → true` starts continuous
dictation and shows the pill (`src/main.rs:130-136`); the second toggle finalizes via
`finalize()` (`src/main.rs:150+`) → `groq::transcribe` → `output::deliver` (`src/output.rs:29`),
which types through `wtype` on this host (`/usr/bin/wtype` present).

---

## (b) Risk table

| # | Failure mode | Likelihood | Mitigation |
|---|---|---|---|
| R1 | **AltGr emits a code other than 100.** `has_altgr_l` (`src/hk.rs:194-202`) requires exactly bit 100, so a device reporting AltGr as KEY_RIGHTMETA (126) or a vendor HID usage in 0xE0–0xE7 is **never grabbed and never matched**. | Low here (this host: 100 is correct on every kbd — kernel keymap shows `0xE6 → Key 100`); Med on some Intel-HID laptops reporting 0xE7 | Add an optional `STATIC_DICTATION_ALTGR_CODE` env, or accept both 100 and 126 in `has_altgr_l` + `decide`. Out of v1 scope; document. |
| R2 | **NumPad0 with NumLock OFF** emits KEY_INSERT (110), which `tracked()` (`src/hk.rs:163`) and `decide()` (`src/hk.rs:242`) do **not** handle → the KP0 toggle silently dies. The comment at `src/hk.rs:18` even says "NumLock off sends KEY_INSERT" but no 110 handling exists. | High (any NumLock-off user) | Live-verified: injecting 110 logged nothing and toggled nothing. Fix: add `KEY_INSERT (110)` as a KP0 alias in `tracked`/`key_name`/`decide`. **Secondary to AltGr+L** but a real bug. |
| R3 | 175 ms global `last_toggle` debounce (`src/hk.rs:24`, `:246`) is **shared** between AltGr+L and KP0, so a legit second toggle <175 ms apart is dropped. | Med (rapid off→on→off) | Accept, or scope the debounce per source. Not a correctness bug — record actual behaviour, don't fail the test. |
| R4 | keyd/interception re-intercepts our re-emitted `static-dictation-kbd` node so grabbed AltGr never reaches apps. | Low **here** (keyd's config `/etc/keyd/kone.conf` has no `rightalt`/`altgr` mapping and keyd's own vkbd is separate) | Add `[ids]` exclusion for `1235:5679` / name `static-dictation-kbd`, or run keyd in tap mode. Not fixable in-process. |
| R5 | Grab contention: another grabber holds `EVIOCGRAB`. | Low (probed: keyd/elogind/mango hold **no** grab on event1/9/20) | Degrades safely: read-only fallback → hotkey still fires but L is **not** swallowed, so the app also receives a literal `l`. |
| R6 | Permissions: node must be `rw` for group `input` (alex ∈ input — verified), `/dev/uinput` `rw` (verified). | Low | If uinput is missing, `UinputKbd::new` returns None (`src/hk.rs:79`) → **all** grabbing disabled, and `has_altgr_l` is never consulted. Ship a udev rule / document `usermod -aG input`. |
| R7 | stdin fallback (`src/hk.rs:437-467`, used when 0 readable nodes or `STATIC_DICTATION_STDIN_HOTKEYS`) **cannot express AltGr+L** — only `""`/`start`/`stop`/`toggle`/`quit`. | Low here (22 nodes armed) | By design; document that the fallback has no chord. |
| R8 | Keyboard unplug/replug: `device_gone` (`src/hk.rs:180`) exits the watcher, rescan re-arms in ≤2 s (`src/hk.rs:400-427`) and re-grabs. A ≤2 s dead window exists. | Med (unplug event) | Expected; handled. |
| R9 | Reboot persistence: **no autostart found** — no `~/.config/autostart`, no `~/.config/systemd/user/*static*`. | High | Add an autostart entry / user service, else the "after reboot" check fails for packaging reasons, not hotkey reasons. |

**Test-harness risk (important):** `scripts/inject_ptt.py` writes **one code, press then release**.
It therefore cannot build an AltGr chord (it releases AltGr before L) and would report a **false
negative** for the very thing being tested. Use `scripts/inject_chord.py` (below).

---

## (c) Test checklist (<10 min)

Run in a Wayland text editor (gedit/kate/mousepad). Keep stderr visible:
`NO_TTS=1 GROQ_API_KEY=… ./target/debug/static-dictation 2>&1 | tee /tmp/hk.log`.

### Scriptable checks (no human hand needed)

| # | Check | How | Pass |
|---|---|---|---|
| S1 | unit state machine | `cargo test hk` | 8 passed |
| S2 | grab actually happened | `grep grabbed /tmp/hk.log` | `event1`, `event9`, `event20` listed |
| S3 | real chord fires toggle | `python3 scripts/inject_chord.py /dev/input/event20 100 38` then `grep -- '-> Toggle' /tmp/hk.log` | `hk: event20 -> Toggle` |
| S4 | app still gets AltGr, not L | 1 s reader on the `static-dictation-kbd` node (`/dev/input/eventNN`) while injecting S3; expect `(100,1) (100,0)`, **no `(38,*)`** | L absent |
| S5 | plain L untouched | `python3 scripts/inject_ptt.py /dev/input/event20 38 0.05`; reader must show `(38,1),(38,0)` | L forwarded |
| S6 | toggle flips recording | S3 → `grep 'toggle event received (recording=false'`; S3 again → `(recording=true` | both lines |
| S7 | no feedback loop | `grep -c 'watching.*eventNN' /tmp/hk.log` (NN = own vkbd node) | `0` |
| S8 | long AltGr hold | `inject_chord_long.py` variant holding AltGr 5 s, then L (see note) | still exactly 1 toggle |
| S9 | hot-plug re-arm | unplug/replug USB kbd; within 3 s `grep 'watching /dev/input/event' /tmp/hk.log \| tail` shows it re-armed | re-armed |
| S10 | debounce sanity | two chords 100 ms apart → exactly one `-> Toggle` | 1 toggle |

Note (S8): AltGr autorepeat keeps `last_event` fresh, so long holds are safe — a plain
`inject_chord.py <node> 100 38` already exercises a normal hold; only add a sleepy variant if you
want to prove the 2 s path.

### Human checks (need eyes/hands)

| # | Check | Steps | Pass |
|---|---|---|---|
| H1 | toggle **in** | focus the editor, press AltGr+L once | pill appears; `recording=true` in log; **no `l` typed** |
| H2 | dictation → Groq | say a short sentence, pause 700 ms | text typed at the cursor by `wtype` + printed to stdout |
| H3 | toggle **out** | press AltGr+L again | pill hides, text stays, `recording=false` |
| H4 | special chars after ×3 | toggle in/out **3 times**, then type `@ € { } [ ] ~ ` with AltGr on this layout | all special chars correct, no stray `l`, no missing chord |
| H5 | concurrent RightCtrl PTT | hold RightCtrl, speak, release; then AltGr+L, speak, AltGr+L | both transcribe; no stuck `recording=true` |
| H6 | fast double toggle | AltGr+L twice within ~150 ms | **expected to collapse to one** (R3) — record actual, not a failure |
| H7 | NumLock-off KP0 | turn NumLock off, press NumPad0 | **R2 — expect NO toggle** (110 unhandled). Record; this is the bug, not a regression |
| H8 | reboot persistence | reboot, start the tool (currently manual), repeat H1 | works once started; **R9 — no autostart yet** |

### scripts/inject_chord.py (new — needed for S3–S6, S8, S10)

```python
#!/usr/bin/env python3
"""Inject a real modifier chord: inject_chord.py <node> <mod> <key> [gap_ms]."""
import os, struct, sys, time
node, mod, key = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
gap = float(sys.argv[4]) / 1000 if len(sys.argv) > 4 else 0.08
EC = struct.Struct('llHHi')
fd = os.open(node, os.O_WRONLY)
def w(ty, c, v):
    t = time.time()
    os.write(fd, EC.pack(int(t), int((t % 1) * 1e6), ty, c, v) + EC.pack(int(t), 0, 0, 0, 0))
w(1, mod, 1); time.sleep(gap)      # AltGr down
w(1, key, 1); time.sleep(gap)      # L down (modifier still held)
w(1, key, 0); time.sleep(gap)      # L up
w(1, mod, 0)                       # AltGr up
os.close(fd)
print(f"chord {mod}+{key} injected into {node}")
```

Writes to a **grabbed** node still work (verified) — the grab does not block writes, only reads by
other clients. Add this file next to `scripts/inject_ptt.py`.

---

## Real bugs found (DO NOT fix in this report; coordinator applies)

**BUG-A — NumPad0 toggle dies when NumLock is off (REAL, verified).**
- Where: `src/hk.rs:18` (comment), `src/hk.rs:163-166` (`tracked`), `src/hk.rs:242-245` (`KEY_KP0` arm).
- Current behaviour: NumLock off makes the kernel deliver **KEY_INSERT = 110**; `decide()` has no
  110 arm and `tracked()` filters it out, so nothing is logged and nothing toggles.
  Live-verified: injecting code 110 into `event20` produced no log line and no `Toggle`.
- Minimal fix: alias `KEY_INSERT` (110) to the KP0 path in `tracked`, `key_name`, and `decide`.

**BUG-A′ (variant gap, no fix needed here) — AltGr code is hard-coded to 100.**
- Where: `src/hk.rs:20`, `src/hk.rs:194-202` (`has_altgr_l`), `src/hk.rs:251`.
- Current behaviour: a device whose AltGr is KEY_RIGHTMETA (126) or a vendor usage other than 100
  is neither grabbed nor matched → **AltGr+L silently inert on that keyboard**.
- Suggest: accept 100 and 126 (or `STATIC_DICTATION_ALTGR_CODE`).

*(The earlier candidate "long AltGr hold fails" was investigated and **refuted** — autorepeat
refreshes the idle timer; see (a). No fix required.)*

---

## Notes / skipped

- Pill render tests fail on this working tree (82 passed / 12 failed, e.g.
  `speech-light too dim: only 57 bright px`); unrelated to hk — `cargo test hk` is green (8 passed).
- Not tested: a keyboard that reports a non-100 AltGr code (BUG-A′), and a keyd tap-mode host.
  R1/R4 rest on reading `/etc/keyd/kone.conf` and the kernel keymap dump, not on hardware.
- No source files were modified.
