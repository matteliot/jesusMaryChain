//! The `cargo sim` demo: its score (the rendering itself is `pedalkit::host`).
//!
//! Plays an original I-vi-IV-V pop progression in G at 126 bpm on a modelled
//! electric guitar (see `guitar`: exact tuning, pick and pickup, strums,
//! human timing), drives the patch's entry points the way the firmware does
//! (128-sample blocks, footswitch events at set times) and writes a stereo
//! float WAV: Honey (clean, ringing strums) -> Psychocandy (fuzz, eighth-note
//! strums) -> feedback armed (a held chord screeches, then the progression
//! over the squall) -> a last chord left to howl and die into the reverb.
//! Prints per-section level stats and fails on NaN or clipping.

use crate::pedalkit::guitar::Note;
use crate::pedalkit::host::{Ev, Score};

// Open-position chord voicings, low string to high.
const G: [f32; 6] = [98.0, 123.47, 146.83, 196.0, 246.94, 392.0];
const EM: [f32; 6] = [82.41, 123.47, 164.81, 196.0, 246.94, 329.63];
const C: [f32; 5] = [130.81, 164.81, 196.0, 261.63, 329.63];
const D: [f32; 4] = [146.83, 220.0, 293.66, 369.99];

enum Local {
    /// Play these notes, strummed this many seconds apart; `ring` lets what's already sounding carry on.
    Play(Vec<Note>, f32, bool),
    Press,
    Hold,
}

fn chord(c: &[f32], vel: f32) -> Vec<Note> {
    c.iter().map(|f| Note::new(*f).vel(vel)).collect()
}

/// An upstroke catches only the top few strings, more lightly.
fn up(c: &[f32], vel: f32) -> Vec<Note> {
    c[c.len().saturating_sub(3)..].iter().map(|f| Note::new(*f).vel(vel)).collect()
}

fn local_score() -> (Vec<(f64, Local, &'static str)>, f64) {
    let mut ev = vec![];
    let q = 60.0 / 126.0; // quarter note
    let e = q / 2.0;
    let mut t = 0.1;
    // Small, repeatable timing slop: nobody plays exactly on the grid.
    let mut seed: u32 = 0x1357_9bdf;
    let mut slop = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f64 / u32::MAX as f64 * 2.0 - 1.0) * 0.006
    };
    let prog: [&[f32]; 4] = [&G, &EM, &C, &D];

    // Honey: one ringing downstroke a beat, a bar per chord, twice round.
    ev.push((0.0, Local::Press, ""));
    for _ in 0..2 {
        for c in prog {
            for k in 0..4 {
                let vel = if k == 0 { 0.8 } else { 0.65 };
                ev.push((t + slop(), Local::Play(chord(c, vel), 0.012, false), ""));
                t += q;
            }
        }
    }
    // Psychocandy: driving eighths, down-up, twice round.
    let fuzz_round = |ev: &mut Vec<(f64, Local, &'static str)>, t: &mut f64, slop: &mut dyn FnMut() -> f64| {
        for c in prog {
            for k in 0..8 {
                let x = if k % 2 == 0 {
                    Local::Play(chord(c, if k == 0 { 0.95 } else { 0.85 }), 0.009, false)
                } else {
                    Local::Play(up(c, 0.6), 0.006, true)
                };
                ev.push((*t + slop(), x, ""));
                *t += e;
            }
        }
    };
    ev.push((t, Local::Press, "psychocandy (fuzz)"));
    for _ in 0..2 {
        fuzz_round(&mut ev, &mut t, &mut slop);
    }
    // Feedback: a G held in front of the amp (the amp keeps the strings
    // ringing) blooms into squall, then the progression over it.
    ev.push((t, Local::Hold, "psychocandy + squall (held G)"));
    ev.push((t + 0.01, Local::Play(G.iter().map(|f| Note::new(*f).vel(0.95).t60(30.0)).collect(), 0.012, false), ""));
    t += 8.0 * q;
    ev.push((t, Local::Play(vec![], 0.0, true), "psychocandy + squall (riff)"));
    fuzz_round(&mut ev, &mut t, &mut slop);
    ev.push((t, Local::Play(D.iter().map(|f| Note::new(*f).vel(0.95).t60(30.0)).collect(), 0.012, false), "last chord, howl"));
    t += 8.0 * q;
    // Mute: the squall dies, the room rings on.
    ev.push((t, Local::Play(vec![Note::new(98.0).vel(0.01).t60(0.02)], 0.0, false), "muted: reverb tail"));
    (ev, t + 3.5)
}

/// The score as pedalkit events.
pub fn score() -> (Score, f64) {
    let (ev, total) = local_score();
    (ev.into_iter().map(|(t, e, l)| (t as f64, e.into(), l)).collect(), total as f64)
}

impl From<Local> for Ev {
    fn from(e: Local) -> Ev {
        match e {
            Local::Play(notes, strum, ring) => Ev::Play(notes, strum, ring),
            Local::Press => Ev::Press,
            Local::Hold => Ev::Hold,
        }
    }
}
