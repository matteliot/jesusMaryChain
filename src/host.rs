//! Host-only demo renderer: `cargo sim [OUT.wav]`.
//!
//! Plays an original I-vi-IV-V pop progression in G at 126 bpm on a modelled
//! electric guitar (see `guitar`: exact tuning, pick and pickup, strums,
//! human timing), drives the patch's entry points the way the firmware does
//! (128-sample blocks, footswitch events at set times) and writes a stereo
//! float WAV: Honey (clean, ringing strums) -> Psychocandy (fuzz, eighth-note
//! strums) -> feedback armed (a held chord screeches, then the progression
//! over the squall) -> a last chord left to howl and die into the reverb.
//! Prints per-section level stats and fails on NaN or clipping.

use crate::guitar::{Note, Player};
use crate::*;

const N: usize = 128;
const SRU: usize = 48_000;
/// The modelled guitar into this patch, matched to the level the patch was voiced for.
const INPUT_TRIM: f32 = 0.6;

// Open-position chord voicings, low string to high.
const G: [f32; 6] = [98.0, 123.47, 146.83, 196.0, 246.94, 392.0];
const EM: [f32; 6] = [82.41, 123.47, 164.81, 196.0, 246.94, 329.63];
const C: [f32; 5] = [130.81, 164.81, 196.0, 261.63, 329.63];
const D: [f32; 4] = [146.83, 220.0, 293.66, 369.99];

enum Ev {
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

fn score() -> (Vec<(f64, Ev, &'static str)>, f64) {
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
    ev.push((0.0, Ev::Press, ""));
    for _ in 0..2 {
        for c in prog {
            for k in 0..4 {
                let vel = if k == 0 { 0.8 } else { 0.65 };
                ev.push((t + slop(), Ev::Play(chord(c, vel), 0.012, false), ""));
                t += q;
            }
        }
    }
    // Psychocandy: driving eighths, down-up, twice round.
    let fuzz_round = |ev: &mut Vec<(f64, Ev, &'static str)>, t: &mut f64, slop: &mut dyn FnMut() -> f64| {
        for c in prog {
            for k in 0..8 {
                let x = if k % 2 == 0 {
                    Ev::Play(chord(c, if k == 0 { 0.95 } else { 0.85 }), 0.009, false)
                } else {
                    Ev::Play(up(c, 0.6), 0.006, true)
                };
                ev.push((*t + slop(), x, ""));
                *t += e;
            }
        }
    };
    ev.push((t, Ev::Press, "psychocandy (fuzz)"));
    for _ in 0..2 {
        fuzz_round(&mut ev, &mut t, &mut slop);
    }
    // Feedback: a G held in front of the amp (the amp keeps the strings
    // ringing) blooms into squall, then the progression over it.
    ev.push((t, Ev::Hold, "psychocandy + squall (held G)"));
    ev.push((t + 0.01, Ev::Play(G.iter().map(|f| Note::new(*f).vel(0.95).t60(30.0)).collect(), 0.012, false), ""));
    t += 8.0 * q;
    ev.push((t, Ev::Play(vec![], 0.0, true), "psychocandy + squall (riff)"));
    fuzz_round(&mut ev, &mut t, &mut slop);
    ev.push((t, Ev::Play(D.iter().map(|f| Note::new(*f).vel(0.95).t60(30.0)).collect(), 0.012, false), "last chord, howl"));
    t += 8.0 * q;
    // Mute: the squall dies, the room rings on.
    ev.push((t, Ev::Play(vec![Note::new(98.0).vel(0.01).t60(0.02)], 0.0, false), "muted: reverb tail"));
    (ev, t + 3.5)
}

pub fn main() {
    let out = std::env::args().nth(1).unwrap_or("target/render/jesusmarychain_demo.wav".into());
    let env = PatchEnv { abi_version: PATCH_ABI_VERSION as u32, user_ctx: core::ptr::null_mut() };
    let mut work = vec![0.0f32; 16];
    unsafe {
        patch_init(&env);
        set_buffer(&env, work.as_mut_ptr());
        for i in 0..PARAM_COUNT {
            set_param(&env, i, get_param_default(&env, i));
        }
    }

    let (mut events, total_s) = score();
    events.sort_by(|a, b| b.0.total_cmp(&a.0));
    let total = (total_s * SRU as f64) as usize;
    let mut guitar = Player::new();

    let mut lo = vec![];
    let mut ro = vec![];
    let mut sections: Vec<(usize, &str)> = vec![(0, "honey (clean + room)")];
    let mut t = 0;
    while t < total {
        while events.last().is_some_and(|(et, _, _)| (*et * SRU as f64) as usize <= t) {
            let (_, e, label) = events.pop().unwrap();
            match e {
                Ev::Press => unsafe { special_action(&env, ACTION_FOOTSWITCH_PRESS) },
                Ev::Hold => unsafe { special_action(&env, ACTION_FOOTSWITCH_HOLD) },
                Ev::Play(notes, strum, ring) => guitar.play(&notes, strum, ring),
            }
            if !label.is_empty() {
                sections.push((t, label));
            }
        }
        let mut l = [0.0f32; N];
        let mut r = [0.0f32; N];
        for x in l.iter_mut() {
            *x = guitar.tick() * INPUT_TRIM;
        }
        unsafe { update_buffers(&env, l.as_mut_ptr(), r.as_mut_ptr(), N as i32) };
        lo.extend_from_slice(&l);
        ro.extend_from_slice(&r);
        t += N;
    }

    let mut bad = false;
    sections.push((lo.len(), ""));
    println!("{:<30} {:>7} {:>8} {:>8} {:>8}", "section", "peak", "rms dB", "L-R dB", "L/R corr");
    for w in sections.windows(2) {
        let (a, name) = w[0];
        let b = w[1].0;
        let (l, r) = (&lo[a..b], &ro[a..b]);
        let peak = l.iter().chain(r).fold(0.0f32, |m, v| m.max(v.abs()));
        let rms = |x: &[f32]| (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt();
        let side: Vec<f32> = l.iter().zip(r).map(|(a, b)| a - b).collect();
        let dot: f64 = l.iter().zip(r).map(|(a, b)| *a as f64 * *b as f64).sum();
        let corr = dot / (rms(l) * rms(r) * l.len() as f64).max(1e-12);
        let to_db = |v: f64| 20.0 * v.max(1e-9).log10();
        println!("{name:<30} {peak:>7.3} {:>8.1} {:>8.1} {corr:>8.2}", to_db(rms(l)), to_db(rms(&side)));
        if l.iter().chain(r).any(|v| !v.is_finite()) || peak >= 1.0 {
            bad = true;
        }
    }
    println!("final LED colour index: {}", unsafe { get_state_idx(&env) });

    if let Some(dir) = std::path::Path::new(&out).parent() {
        std::fs::create_dir_all(dir).unwrap();
    }
    write_wav(&out, &lo, &ro);
    println!("wrote {out} ({:.1} s)", lo.len() as f64 / SRU as f64);
    if bad {
        eprintln!("error: NaN or full-scale output");
        std::process::exit(1);
    }
    let _ = (get_buffer_size, get_param_min, get_param_max, is_param_enabled, get_param_name, get_param_unit);
}

fn write_wav(path: &str, l: &[f32], r: &[f32]) {
    let bytes = (l.len() * 8) as u32;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + bytes).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&48_000u32.to_le_bytes());
    b.extend_from_slice(&(48_000u32 * 8).to_le_bytes());
    b.extend_from_slice(&8u16.to_le_bytes());
    b.extend_from_slice(&32u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&bytes.to_le_bytes());
    for (a, c) in l.iter().zip(r) {
        b.extend_from_slice(&a.to_le_bytes());
        b.extend_from_slice(&c.to_le_bytes());
    }
    std::fs::write(path, b).unwrap();
}
