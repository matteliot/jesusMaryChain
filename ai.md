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

3. **Buffers are zero-initialized in `.bss` and explicitly zeroed in `patch_init` before use** (rather than shipped as zeros inside the patch image). The delay lines (and reverb, where there is one) move out of the patch state into a separate `Bufs` static that is all zero at rest, so it lands in `.bss` instead of being stored as zeros in the `.endl` (runtime-length reverb combs/allpasses start with length 0 and get their lengths in init). The `Global` cell is generic; `tick` and the methods that use a delay line take `&mut Bufs`. `patch_init` then clears each buffer static in place with `core::ptr::write_bytes` (no heap or `calloc` in `no_std`, and a zeroed copy would be a stack temporary) before `init`: the loader should already have cleared `.bss`, but nothing promises it, and a NaN left in a feedback loop never decays. The firmware working buffer stays unused.

4. **Ship the built patch in a `release/` directory at the repo root** (`release/jesusmarychain.endl`, a copy of `target/endless/jesusmarychain.endl` from `./build-endl.sh`), committed to git, so a user can download it from GitHub and copy it straight to the pedal without building. README has an Install section saying so. Rebuild and recopy it whenever the patch changes.

5. **Use the shared `pedalkit`, so this pedal can also live inside `../superPedal` (one patch that combines every pedal, picked on its left knob).** In the user's words: "change them all to use the pedalkit you just made and feel free to modify pedalkit as needed to make it more usable by these repo's- your idea where each pedal is more settings the values for the pedalkit to use it a great one, try to use it as much as it makes sense", and "make sure that the button and knob controls on the original projects aren't changed. only the superpedal should change how the knobs work". So every knob, default, footswitch action and LED colour stays exactly as before.

### Implementation decisions (needed to recreate requirement 5)

- **pedalkit is vendored.** `src/pedalkit/` is an exact copy of `../superPedal/src/pedalkit/`, the master copy; `../superPedal/sync.sh kit` refreshes it and `../superPedal/sync.sh check` confirms the copies match. Don't edit it here. It holds the Endless ABI (`abi.rs`), the f32 DSP toolkit (`dsp.rs`, the old `src/dsp.rs` plus additions), the modelled demo guitar (`guitar.rs`, unchanged), the `Pedal` trait and the `Runner` behind the firmware entry points (`pedal.rs`), `endless_patch!` (`endless.rs`: entry points, patch header, panic handler), delay memory (`pool.rs`), the shared effect blocks and the host demo renderer (`host/`), and `tools/` (`build-endl.sh`, `check_endl.py`, `make-prompts.sh`).
- **The pedal is a module, `src/jesus_mary_chain/`,** self-contained apart from `crate::pedalkit` (it refers to its own files with `super::`), so superPedal carries an identical copy as `src/pedals/jesus_mary_chain/`. One struct per file: `fuzz.rs` (Psychocandy fuzz), `mod.rs` (The Jesus and Mary Chain's *Psychocandy* tone), `patch.rs` (The whole patch), `presets.rs` (The Jesus and Mary Chain's settings). Every number that shapes the sound is in `presets.rs`; `patch.rs` is the `Patch` implementing `Pedal` (its knob table `PARAMS`, `init`, `set_knob`, `press`, `hold`, `tick`, `led`) and wires the blocks together. Blocks used by more than one pedal come from pedalkit, each set by a settings struct or constants from `presets.rs`.
- **`src/main.rs`** only declares `mod pedalkit; mod jesus_mary_chain;`, calls `endless_patch!(kit = crate::pedalkit, pedal = crate::jesus_mary_chain::Patch);` and, on the host, runs `host::main`.
- **Delay memory** comes from a `Pool`: a static `.bss` array of `Patch::POOL` floats, zeroed in place by the runner in `patch_init` before the pedal uses it. This supersedes requirement 3's `Bufs`/`Delay<N>` statics: the buffers are still a static `.bss` array zeroed in place in `patch_init`, now handed out exactly sized by the pool. The firmware working buffer stays unused; if it is missing, the LED shows red as before.
- **Host demo:** `src/host/`: `mod.rs` (runs the render and any extra analysis), `score.rs` (what the demo plays, unchanged). It renders through `pedalkit::host` (128-sample blocks via the entry points, per-section stats, float WAV). `PEDAL_DUMP=<dir>` also writes the input and the timed events.
- **`build-endl.sh`** calls `src/pedalkit/tools/build-endl.sh <bin> jesusmarychain`: builds, makes `target/endless/jesusmarychain.endl`, checks for f64 instructions (inside function symbols only), checks the 512 KB limit, and validates the header against the SDK layout with `tools/check_endl.py`. `release/jesusmarychain.endl` is that file.
- **Verified:** `cargo sim` renders byte-for-byte identically to the pre-pedalkit code, along with every analysis the demo prints.
