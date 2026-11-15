//! H1 harness: in-process frame dump at fixed 100 ms wall intervals.
//!
//! `STATIC_DICTATION_DUMP_FRAMES=<dir>` (set at process start) makes `main`
//! spawn a headless dumper thread that renders through the pill's REAL
//! pipeline — the same `vecmentor::Sim` physics + `canvas::render_pill`
//! rasterizer the live Wayland path and the `preview.rs` offscreen tests use
//! — and writes one composited PNG per fixed 100 ms wall tick to
//! `<dir>/frame-%04d.png`, numbered by dump sequence (not wall clock) so slow
//! PNG I/O can never alias the cadence. This beats grim's ~0.3 s/shot
//! aliasing of the 1.2–2.8 s idle breathing cycles (the motion critic's
//! round-2 complaint).
//!
//! Frames are composited at alpha 1.0, grow 1.0 (the posed pill) and the Sim
//! is fed silence (or a synthetic voiced mound with
//! `STATIC_DICTATION_DUMP_SPEECH=1` for active-frame QA). Env unset →
//! this module never runs; behavior is byte-identical to before.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::pill::canvas::{render_pill, SURF_H, SURF_W};
use crate::pill::vecmentor::{Sim, NUM_BARS};

/// Live pill cadence: the pill is updated at ~30 Hz (30 ms audio frames in
/// main.rs), so the dump's Sim advances at the same rate.
const FEED: Duration = Duration::from_millis(33);
/// Dump interval: fixed 100 ms wall ticks → frame-%04d.png at ~10/s.
const DUMP: Duration = Duration::from_millis(100);
/// Render scale 2 = the same 440×128 buffer geometry the compositor shows
/// (Mango advertises scale 2; matches the ×2 crops in capture_pill.sh).
const SCALE: u32 = 2;

/// Spawn the dumper iff the env flag names a directory. Returns immediately;
/// the thread runs until process exit (nothing joins it; main exits hard).
pub fn maybe_spawn() {
    let Some(dir) = std::env::var_os("STATIC_DICTATION_DUMP_FRAMES") else {
        return;
    };
    let dir = PathBuf::from(dir);
    if std::fs::create_dir_all(&dir).is_err() {
        eprintln!("dump: cannot create {dir:?}; frame dump disabled");
        return;
    }
    if std::thread::Builder::new()
        .name("frame-dump".into())
        .spawn(move || run(dir))
        .is_err()
    {
        eprintln!("dump: thread spawn failed; frame dump disabled");
    }
}

fn run(dir: PathBuf) {
    let speech = std::env::var_os("STATIC_DICTATION_DUMP_SPEECH").is_some();
    eprintln!(
        "dump: STATIC_DICTATION_DUMP_FRAMES active — {} frames (10/s, scale {SCALE}) → {}",
        if speech { "voiced" } else { "idle-pose" },
        dir.display()
    );
    let mut sim = Sim::new(crate::pill::vecmentor::wave_params());
    let silence = [0.0f32; NUM_BARS];
    let mut next_push = Instant::now() + FEED;
    let mut next_dump = Instant::now() + DUMP;
    let mut seq: u32 = 0;
    let mut feed: u32 = 0;
    loop {
        let now = Instant::now();
        if now >= next_push {
            if speech {
                sim.push(0.8, &speech_bars(feed), true);
            } else {
                sim.push(0.0, &silence, false);
            }
            feed += 1;
            next_push += FEED;
        }
        if now >= next_dump {
            // Sequence-numbered: a slow PNG write shifts the next deadline,
            // never renumbers frames (no aliasing of the 10 Hz cadence).
            let path = dir.join(format!("frame-{seq:04}.png"));
            write_png(&path, sim.bars(), seq as f32 * 0.1);
            seq += 1;
            next_dump += DUMP;
        }
        let next = next_push.min(next_dump);
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
    }
}

/// QA-only voiced feed: a reference-like ASYMMETRIC speech envelope with a
/// slow fluid swell — gradual left ramp, apex right of centre, steep right
/// falloff. The renderer adds only a gentle asymmetric illumination on top.
fn speech_bars(feed: u32) -> [f32; NUM_BARS] {
    let mut bars = [0.0f32; NUM_BARS];
    let n = (NUM_BARS - 1) as f32;
    let swell = (feed as f32 * 0.05).sin() * 0.06;
    for (i, b) in bars.iter_mut().enumerate() {
        let u = i as f32 / n;
        let d = (u - 0.56) / 0.34;
        let sig = if u < 0.56 { 1.4 } else { 4.5 };
        *b =
            (0.12 + 0.88 * (-d * d * sig).exp() + swell * (1.0 - d.abs()).max(0.0)).clamp(0.0, 1.0);
    }
    bars
}

/// Render one pill frame through the REAL renderer and write it as PNG.
/// Alpha 1.0, grow 1.0: the posed capsule, so idle runs still dump the pose.
fn write_png(path: &std::path::Path, bars: &[f32], t_secs: f32) {
    let (w, h) = (SURF_W * SCALE, SURF_H * SCALE);
    let mut buf = vec![0u8; (w * h * 4) as usize];
    render_pill(&mut buf, w, h, bars, t_secs, 1.0, 1.0);
    // BGRA (premultiplied) → RGB composited over the live host navy, so a
    // dump frame measures exactly like a compositor screenshot.
    const HOST: [u32; 3] = [17, 33, 66];
    let mut rows = Vec::with_capacity((w * h * 3) as usize);
    for px in buf.as_chunks::<4>().0 {
        let (b, g, r, a) = (px[0] as u32, px[1] as u32, px[2] as u32, px[3] as u32);
        let inv = 255 - a;
        for (c, host) in [(r, HOST[0]), (g, HOST[1]), (b, HOST[2])] {
            rows.push((c + host * inv / 255).min(255) as u8);
        }
    }
    let _ = write_png_file(path, w, h, &rows);
}

/// Minimal PNG writer: 8-bit RGB, filter byte 0 per row, zlib stream of
/// stored (uncompressed) deflate blocks — no dependency, ~50 KB per frame.
fn write_png_file(path: &std::path::Path, w: u32, h: u32, rgb: &[u8]) -> std::io::Result<()> {
    let stride = w as usize * 3;
    let mut raw = Vec::with_capacity((stride + 1) * h as usize);
    for y in 0..h as usize {
        raw.push(0u8); // filter type 0 (None)
        raw.extend_from_slice(&rgb[y * stride..(y + 1) * stride]);
    }

    let mut out = Vec::with_capacity(raw.len() + 256);
    out.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit depth, color type 2 (RGB)
    chunk(&mut out, b"IHDR", ihdr);

    // zlib: header + stored deflate blocks (≤65535 bytes each) + adler32.
    let mut idat = vec![0x78, 0x01];
    let mut off = 0usize;
    while off < raw.len() {
        let n = (raw.len() - off).min(65535);
        let last = off + n == raw.len();
        idat.push(u8::from(last));
        idat.extend_from_slice(&(n as u16).to_le_bytes());
        idat.extend_from_slice(&(!(n as u16)).to_le_bytes());
        idat.extend_from_slice(&raw[off..off + n]);
        off += n;
    }
    idat.extend_from_slice(&adler32(&raw).to_be_bytes());
    chunk(&mut out, b"IDAT", idat);
    chunk(&mut out, b"IEND", vec![]);
    std::fs::write(path, &out)
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: Vec<u8>) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(&data);
    out.extend_from_slice(&crc32(kind, &data).to_be_bytes());
}

fn crc32(a: &[u8], b: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &x in a.iter().chain(b.iter()) {
        crc ^= u32::from(x);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}
