# jesusMaryChain

A [Polyend Endless](https://polyend.com/endless/) patch for the most Jesus and Mary
Chain guitar tone possible: sweet pop chords buried under fuzz, feedback and reverb.
It's written in `no_std` Rust against the patch ABI of Polyend's
[FxPatchSDK](https://github.com/polyend/FxPatchSDK) (ABI 0x000B), so no ARM GCC is needed.

Two voices run all the time and the footswitch crossfades between them:

- **Honey** (regular): a bright, jangly clean amp with gentle compression, a
  250 Hz dip and a +5 dB lift at 3.6 kHz. It's drenched in a big Spector-style
  room. Think *Just Like Honey* and *Darklands*.
- **Psychocandy** (distorted): a thin, treble-pushed fuzz into a second fuzz.
  Stage 1 is Super-Fuzz-style saturation plus a full-wave rectified copy, which
  adds an octave up. Stage 2 is a hard clipper. Then a 650 Hz scoop and a +6 dB
  bite at 2.8 kHz: the *Psychocandy* wall of noise.

Both go through a small open-back combo cab sim and a stereo Freeverb-style
reverb (22 ms pre-delay). The reverb is shared, so its tail carries across a
switch. Input comes from the left jack.

**Feedback squall**: hold the footswitch to arm it. In Psychocandy, a sustained
chord screeches into feedback within about a second. The amp takes the string
over, so the picked sound ducks under the screech. The pitch of the squall
wanders by itself between about 1.5 and 3.6 kHz, and it dies when you mute the
strings.

| Control | Function |
|---|---|
| Left knob | **Fuzz**: fuzz gain, about 20 to 50 dB |
| Mid knob | **Reverb**: wet level and room size. CCW is dry; CW is an endless wash |
| Right knob | **Level**: output, unity at 3 o'clock |
| Footswitch press | **Honey ↔ Psychocandy**. Honey sits about 5 dB under the fuzz |
| Footswitch hold | **Feedback squall** armed/off. It only sounds in Psychocandy |

| LED | Feedback off | Feedback armed |
|---|---|---|
| Honey | light yellow | beige |
| Psychocandy | magenta | red |

## Build

```sh
rustup target add thumbv7em-none-eabihf --toolchain nightly
rustup component add llvm-tools --toolchain nightly
cargo build --release        # ELF for the pedal (default target is thumbv7em-none-eabihf)
./build-endl.sh              # build + make target/endless/jesusmarychain.endl, check f64 use and size
./build-endl.sh --deploy     # ...and copy to /Volumes/ENDLESS
```

## Audition on your computer

```sh
cargo sim                    # -> target/render/jesusmarychain_demo.wav
```

This renders an original I–vi–IV–V progression in G at 126 bpm on a modelled
electric guitar (`src/guitar.rs`). It plays Honey (one ringing strum per beat),
then Psychocandy (eighth-note down/up strums). Then it arms the squall: a held G
screeches, the progression plays over it, a final D howls, and the strings are
muted into the reverb tail. Then it prints the level and stereo width of each
section.
