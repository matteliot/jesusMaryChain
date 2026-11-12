# ai.md — Project Requirements Log

This file records every requirement given for the **jesusMaryChain** project, so the whole project can be recreated from scratch by handing this file to an AI assistant as the prompt.

It is kept up to date as new requirements are given. Requirements are listed in the order they were given; later entries override earlier ones where they conflict (and the earlier entry is marked as superseded).

---

## Meta / Process Requirements

1. Maintain this `ai.md` file at the repo root. Its purpose is to keep a list of all requirements so the entire project can be recreated from this file alone. Update it as work progresses, whenever a new requirement is given. Keep lines unwrapped (wrap only around 200 characters at a natural break).

## Project Requirements

2. **In the existing `jesusMaryChain` directory, build a Polyend Endless patch for "the most Jesus and Mary Chain possible guitar tone": one regular tone and one distorted tone.** Built the same way as the sibling projects (`../nirvana`, `../soundGarden`), with `cargo build --release`.

### Implementation decisions (needed to recreate requirement 2)

- **Toolchain and packaging (copied unchanged from `../nirvana`).** `no_std` Rust, edition 2024, nightly, `thumbv7em-none-eabihf`. Copied: `src/abi.rs`, `src/dsp.rs`, `src/guitar.rs` (host-only guitar model), `patch.ld`, `build.rs`, `.cargo/config.toml` (`-fp64`, `cargo sim` alias), `rust-toolchain.toml`, `.gitignore`. `build-endl.sh` makes `target/endless/jesusmarychain.endl`, checks for f64 instructions and the 512 KB limit; `--deploy` copies to `/Volumes/ENDLESS`.
- **Input:** left jack, HP 80 Hz. Envelope (1 ms attack, 80 ms release) drives a gate on the fuzz voice: opens at 0.0010, closes at 0.0004, 2 ms open, 120 ms close.
- **Honey (regular voice):** HP 100 Hz → peak compressor (as nirvana's verse) → `tanh(1.6x)·0.7` → peak 250 Hz Q0.8 −2.5 dB → peak 3.6 kHz Q0.8 +5 dB. Gain −5 dB + 23 dB makeup (lands ~4.5 dB RMS under the fuzz).
- **Psychocandy (distorted voice):**
  1. HP 140 Hz; add 2× the content above 900 Hz (one-pole) as treble emphasis.
  2. Fuzz gain 20–50 dB (Fuzz knob). Stage 1: `a = tanh(u); a + 0.45·|a|` (rectified octave-up), 2× oversampled (midpoint average).
  3. HP 160 Hz (removes the octave's DC), LP 6.5 kHz.
  4. Stage 2: +16 dB into cubic hard clipper (`1.5c − 0.5c³`, flat beyond |u| = 1.5), 2× oversampled.
  5. Peak 650 Hz Q0.7 −5 dB, peak 2.8 kHz Q1 +6 dB, LP 8.5 kHz. Multiplied by the gate.
- **Feedback squall (footswitch hold arms it; only active in Psychocandy while the gate is open):** fuzz output → band-pass (Q 7, centre `2300·2^(0.65·w)` Hz, `w = 0.6·sin(0.11 Hz) + 0.4·sin(0.29 Hz)`, retuned every 32 samples) → 263-sample delay → added to the fuzz input × ramp × 1.5 / stage-1 gain. The ramp blooms with τ = 0.4 s and dies with τ = 0.25 s. The dry input is ducked by `1 − 0.6·ramp` (without ducking, the louder string hogs the clippers and the squall is inaudible).
- **Mix and output:** 12 ms crossfade between voices (mono) → open-back combo cab: HP 85, peak 120 Hz +2 dB, peak 2.5 kHz +2 dB, peak 5 kHz −2 dB (Q2), LP 7 kHz Q0.7, LP 10 kHz Q0.6. Then the shared stereo reverb: 22 ms pre-delay (1056 samples), Freeverb (8 combs at 1214…1759 samples, 4 allpasses at 605/480/371/245, right channel +25 samples, input ×0.015, output ×3, damping 0.38). Room feedback `0.80 + 0.165·Reverb`, wet `Reverb × (1.15 Honey … 0.8 Psychocandy)` following the crossfade. Output `(dry + wet·rev) × Level² (unity at 0.75) × 0.16`, then `tanh × 0.98`.
- **Controls:** Fuzz (default 0.75), Reverb (default 0.55), Level (default 0.75). Press: Honey ↔ Psychocandy (starts in Psychocandy). Hold: feedback armed on/off. LED: Honey light yellow / beige, Psychocandy magenta / red (feedback off / armed). Red also means the working buffer is missing.
- **Host audition:** `cargo sim` renders an original I–vi–IV–V progression in G (open G, Em, C, D voicings) at 126 bpm, input ×0.6, ±6 ms timing slop: Honey (quarter-note strums, 2 rounds) → Psychocandy (eighth-note down/up strums, upstrokes on the top 3 strings, 2 rounds) → hold: G sustained with T60 30 s for 2 bars → one round over the squall → D sustained 2 bars → mute → 3.5 s tail. Output `target/render/jesusmarychain_demo.wav`, with per-section peak/RMS/side/correlation stats; it fails on NaN or full-scale output.
