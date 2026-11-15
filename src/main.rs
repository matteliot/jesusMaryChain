//! jesusmarychain: a Polyend Endless patch for the most Jesus and Mary Chain
//! guitar tone possible: sweet pop under a wall of fuzz, feedback and reverb.
//!
//! Two voices run all the time and the footswitch crossfades between them:
//!   Honey        bright, jangly clean amp with gentle compression, drenched in
//!                a big, dark Spector-style room ("Just Like Honey", Darklands)
//!   Psychocandy  thin, treble-pushed fuzz into fuzz: a Super-Fuzz-style stage
//!                with a rectified octave-up, then a hard clipper, then a
//!                scooped, biting EQ (the Psychocandy wall of noise)
//! then a small open-back combo cab sim and a stereo reverb shared by both.
//! Holding the footswitch arms a feedback squall: with Psychocandy on, a
//! sustained chord screeches into feedback whose pitch wanders on its own,
//! like a guitar held up against a cranked amp.
//!
//! Controls
//!   Left knob   Fuzz    (fuzz gain, ~20 to 50 dB)
//!   Mid knob    Reverb  (wet level and room size; CW = endless wash)
//!   Right knob  Level   (output; unity at 3 o'clock)
//!   Footswitch press  Honey <-> Psychocandy (the clean sits ~5 dB under the fuzz)
//!   Footswitch hold   Feedback squall on/off (sounds in Psychocandy only)
//!   LED  Honey: light yellow | Psychocandy: magenta | with feedback armed: beige | red
//!
//! The sound lives in `jesus_mary_chain` (built from `pedalkit` blocks set by its
//! `presets`); the entry points come from `pedalkit::endless_patch!`.
//! Host builds (`cargo sim`) render a demo riff through the same code; see `host`.

#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

/// Reusable blocks (effects, DSP, the Endless ABI); see its docs.
mod pedalkit;

/// The pedal itself.
mod jesus_mary_chain;

endless_patch!(kit = crate::pedalkit, pedal = crate::jesus_mary_chain::Patch);

#[cfg(not(target_os = "none"))]
mod host;

#[cfg(not(target_os = "none"))]
fn main() {
    host::main();
}
