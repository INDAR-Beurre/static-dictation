# Footprint & Fluidity Audit

Read-only audit of `src/` + `Cargo.toml` at commit range "mid-edit" (7 files dirty).
Hard requirement: total footprint ≤ 50 MB (binary + runtime + assets).

---

## 1. SIZE

### Measured now

| Artifact | Bytes | On disk |
|---|---|---|
| `target/release/static-dictation` (`opt-level=3, lto=true, strip=true`) | 3,757,344 | **3.6 MiB** |
| …same, ELF `.text` | 3,646,818 | — |
| …same, `.data` + `.bss` | 103,072 + 13,080 | — |
| `target/debug/static-dictation` (debug, present in tree) | 77,438,688 | **73.9 MiB** |
| `~/.config/static-dictation/` (runtime state: `api_key`, 56 B) | 8.0 KiB dir | **8 KiB** |
| Embedded runtime assets (fonts/images) | **none** — no `include_bytes!`, no `.ttf/.otf/.png` in `src/` | 0 |
| Working-tree non-source dirs (`live-proof/` 41 MiB, `critique/` 3.3 MiB) | not shipped | 0 |

Release binary is 3.6 MiB. Config state is 8 KiB. Assets are zero.
**Total shipped runtime footprint ≈ 3.6 MiB — 14× under the 50 MB ceiling.**

### Where the 3.6 MiB goes

`nm --size-sort` on a non-stripped, `lto=false` build (`.text` 2.91 MB, `.rodata` 0.54 MB,
`.eh_frame` 0.37 MB), aggregated by namespace:

| Contribution | ≈ size | Share of symbols | Note |
|---|---|---|---|
| `ring` (native crypto: AES-NI, chacha20-poly1305, nistz256 tables) | ~0.60 MB | ~15% | pulled by `rustls-tls` for HTTPS to Groq |
| `.eh_frame` unwind tables | 0.37 MB | (section) | every Rust fn keeps unwind metadata |
| `core`/`std`/`alloc` generic instantiations | ~0.50 MB | ~12% | normal for a networked binary |
| `rustls` + `webpki` + `hyper` + `tokio` + `reqwest` | ~0.10 MB | ~3% | TLS + HTTP/1.1 over a 260-line dependency chain |
| `wayland_client` + SCT + wayland-protocols | ~0.02 MB | <1% | tiny |
| `static_dictation` (all our code, incl. canvas/vecmentor/tsar) | **0.039 MB** | 1% | our 6,056 lines are ~1% of the binary |
| `panic` strings / debuginfo | see below |

Dependency chain: 234 locked crates, 317 tree nodes — dominated by `reqwest → hyper →
hyper-util → tokio → rustls → ring → webpki-roots`.

### Top 3 size reductions (if we ever need headroom)

1. **`panic = "abort"` in `[profile.release]`** — measured: 3,493,832 → **3,160,632 B,
   −333 KB (−9.5%)** (independent of `codegen-units`; `codegen-units=1` alone measured
   +65 KB). Cost: no unwinding — `AliveGuard` relies on Drop-on-unwind, so set
   `PillApp` alive flag in `LayerShellHandler::closed` instead. Not needed at 3.6 MiB.
2. **Replace `reqwest` + `rustls`/`ring` with `ureq` + `rustls`** (or use Groq over a
   plain `TcpStream` with a hand-rolled multipart body): drops hyper/tokio/h2/hyper-util
   and their TLS glue. Est. **−400…600 KB**. Only worth it if the tool ever ships a
   vendor CA bundle.
3. **`codegen-units = 2`→`1` is a wash** (+65 KB measured). Real remaining lever is
   `opt-level = "s"` (est. −15…20% `.text`, ~−450 KB) at a small render-time cost —
   acceptable for a 6 ms rasterizer with 100 ms of slack (see §2).

### Is there ANY path to >50 MB? — No.

* Shipped release binary: 3.6 MiB. `strip = true` is in `[profile.release]`; `file`
  reports `stripped`; `readelf -S` shows **no `.debug_*` and no `.symtab`**.
* The only >50 MB artifact in the tree is **`target/debug/static-dictation` (73.9 MiB)**
  — a dev-only build that is `.gitignore`d (`target/`) and never shipped. A CI gate that
  size-checks the *release* path is sufficient; the debug path cannot leak into a release.
* Panic strings: small (included in the 3.6 MiB). Debuginfo: none.
* Even the 95 MiB working tree is `live-proof/` PNGs (41 MiB) — not installed.

---

## 2. FLUIDITY

### Cadence — the important correction

| Clock | Source | Multiplier |
|---|---|---|
| `main.rs FRAME = 480` @ 16 kHz = **30 ms** audio frame | `src/main.rs:23` | 33.3 Hz |
| `main.rs` idle poll | `sleep(10 ms)` | only when no frame is pending |
| `pill::update` → 30 ms audio frames | `src/pill.rs:14` contract | ~33 Hz, **not** 100 ms |
| `tsar::TICK = 16 ms` render/fade tick | `src/pill/tsar.rs:100` | 62.5 Hz |
| `dump.rs DUMP = 100 ms` | `src/dump.rs:28` | **QA harness only, off by default** |

The 100 ms figure is the *headless frame-dump harness*, not the live animation. The live
pill redraws on every `Ctl::Update` (≈33 Hz) and on every 16 ms tick while fading/growing.
That is adequate for the animation: the entrance spring is ω=21 rad/s (300 ms period), so
33 Hz samples it ~10× per period — no visible temporal aliasing.

### Render cost — measured (this machine: i7-1185G7, 8 threads)

In-tree micro-benchmark of the real `canvas::render_pill` path (added to a scratch copy,
600 iterations after warm-up):

| Pose | Buffer | Per frame | Headroom |
|---|---|---|---|
| Posed pill, bars + dots (scale 2, 440×128) | 225 KB | **6.8–7.6 ms** | 131–147 fps |
| Same, scale 1 (220×64) | 56 KB | 2.47 ms | 405 fps |
| Body + shadow only, no bars/dots | 440×128 | 3.28 ms | 305 fps |
| Collapsed twin (`grow=0`) | 440×128 | 1.30 ms | 770 fps |

Worst case (entrance + full waveform at scale 2) is **~7 ms against a 30 ms budget** —
~4.3× headroom, and the compositor scale is only 2 on Mango. A modest iGPU-adjacent
laptop CPU (2–3× slower than this i7) still lands around 15–22 ms, i.e. still inside the
30 ms audio frame. `contrib` analysis: SDF body + shadow ≈ 3.3 ms, 27 bar capsules +
bloom ≈ 3.5 ms, leader dots ≈ 0.4 ms (fast-path x-range).

### Compositing

**CPU (software, `wl_shm`)** — `tsar.rs` uses `SlotPool` + `wl_shm::Format::Argb8888` and
blits via `create_buffer`/`attach_to`. No GPU path. That is the load-bearing 6.8–7.6 ms
above. It is a deliberate, correct choice: the surface is 220×64 logical.

### Allocations in the hot loop

* `render_pill` — **one `Vec<Bar>` with exact `with_capacity(bars.len())` per frame** plus
  a small `Vec<(f32,f32)>` for dots. Bounded (27 + ≤40), so the allocator reuses freed
  blocks; no growth churn. **No `format!`, no `String`, no `to_vec`, no `clone`** anywhere
  in the pixel loop or the sim step (`vecmentor::Sim` is fixed-size POD, `Copy`).
* `tsar::draw` — `SlotPool::create_buffer()` per frame (buffer *reuse* from the pool, not
  a fresh mmap; pool is pre-allocated at `SURF_W*SURF_H*4*4` = 225 KB ×4).
* The true per-frame cost is the ~225 KB pixel write + 56 K SDF evaluations, not allocation.

### Dropped frames on the entrance spring (ζ=0.45)

No. The spring is integrated with **semi-implicit Euler at a fixed `dt = TICK = 16 ms`**
(`step_grow_tick(..., TICK.as_secs_f32())`), and draw + sim run in the *same* loop round as
the tick. A frame that takes 7 ms simply makes that round 7 ms and the next one 9 ms —
`dt` is a constant, so spring amplitude and ζ are unaffected by render jitter. The spring
cannot blow up: k≈0.338 ≤ c=0.56 (stable region), and `grow` is clamped 0…1.25.

The documented failure mode was the **opposite** — a frame-callback flood
(`frame() → draw() → request callback`) running at 180+ Hz and starving the 16 ms fade
tick, stretching a 320 ms fade into ~2 s. That is already fixed: `draw()` never requests a
frame callback (`src/pill/tsar.rs`, `frame()` handler + the `frame_callback_*` contract
tests).

### Verdict

**Animation is smooth on a modest laptop CPU, with ~4× headroom, and does not drop frames
on the entrance spring.** No change is required for the stated target hardware.

If a low-end target ever needs it, two changes buy margin (in order):

1. **Cut `GLOW_REACH` (18 px) or bound the bloom to a shorter range.** Bloom is the single
   largest branch in the pixel loop; halving its reach cuts the per-pixel bar work roughly
   in half → est. −1.5 ms of the 3.5 ms waveform time.
2. **Skip `draw()` when the pose is unchanged.** `need_redraw` is set by every `Ctl::Update`
   even when the bars are at rest; gating on "any bar moved > ε" would idle at ~0 cost
   between utterances instead of 33 full-surface redraws/s.

Do **not** "fix" this by raising `TICK` or lowering the audio frame rate — both would trade
visible smoothness for a problem that does not exist.

---

## 3. Keep under 50 MB forever

### CI size gate (suggested)

Guard on the **release** binary only — that is the only shipped artifact.

```yaml
- run: cargo build --release
- run: |
    sz=$(stat -c%s target/release/static-dictation)
    echo "release binary: $((sz/1024)) KiB"
    test "$sz" -lt 15728640 || { echo "FAIL: >15 MiB (headroom vs 50 MB budget)"; exit 1; }
- run: |
    # refuse an accidentally-unstripped / debuginfo build
    ! readelf -S target/release/static-dictation | grep -qE '\.debug_|\.symtab' \
      || { echo "FAIL: debug symbols in release binary"; exit 1; }
```

15 MiB gate = **4× the current 3.6 MiB**, so it trips on a regression (debug build, lost
`strip`, a giant new dependency) long before the 50 MB requirement is at risk.

### Checklist

- [ ] `[profile.release]` keeps `lto = true` **and** `strip = true` — both are load-bearing;
      dropping `strip` alone adds back ~0.3 MB of symtab, dropping LTO much more.
- [ ] Never ship `target/debug/*`. Add the CI gate above rather than trusting `.gitignore`.
- [ ] Re-check `cargo tree --edges normal` on any dependency PR: the whole HTTP/TLS tail
      (`reqwest → hyper → hyper-util → tokio → rustls → ring → webpki-roots`, 234 crates)
      is the only large surface. Keep `reqwest` on `default-features = false` with exactly
      `["blocking","multipart","rustls-tls"]` — enabling `default-tls`/`native-tls`/`json`
      drags in OpenSSL or ICU and can add multiple MB.
- [ ] Keep `smithay-client-toolkit` feature-minimal (`wayland_client` + protocols only);
      the toolkit's xdg/wgpu/egl features are unwired and would balloon the binary.
- [ ] **No embedded fonts or images.** The pill is drawn procedurally (SDF + Bayer dither)
      and `src/` contains zero `include_bytes!`/`include_str!` of binary assets. Adding one
      photographic asset (>15 MB) is the *only realistic path to breaking 50 MB* — gate it.
- [ ] Runtime state stays tiny: `~/.config/static-dictation/` is 56 B today. Never cache
      audio buffers, model weights, or frame dumps there (`dump.rs` writes to a
      caller-supplied dir only, and only behind `STATIC_DICTATION_DUMP_FRAMES`).
- [ ] Keep the QA harnesses (`dump.rs`, `preview.rs`) behind `#[cfg(test)]` / env flags so
      they never inflate the shipped code path.

---

## Audit method / reproducibility

* Release build: `cargo build --release` → 3.6 MiB (32 s, warm).
* Section + symbol attribution: temporary `[profile.sizemeasure]` (`lto=false, strip=false`)
  then `size -t`, `readelf -SW`, `nm --print-size --size-sort --demangle`. Profile removed
  after measurement; `Cargo.toml` restored byte-for-byte.
* Render timing: scratch copy of `src/` with a `bench_render_scale2` test calling the real
  `canvas::render_pill`, 600 iterations @440×128 scale 2, 50-iteration warm-up. All numbers
  are single-run wall clock on an i7-1185G7 (AVX-512) — treat as an upper bound on quality,
  not a floor.
* No source file was modified; only `critique/footprint-fluidity.md` was written.
