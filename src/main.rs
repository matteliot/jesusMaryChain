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
//! Host builds (`cargo sim`) render a demo riff through the same code; see `host`.

#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg_attr(not(target_os = "none"), allow(dead_code))]
mod abi;
#[allow(dead_code)]
mod dsp;

use core::cell::UnsafeCell;
use core::f32::consts::TAU;

use abi::*;
use dsp::*;

// ---------------------------------------------------------------- voicing

/// Fuzz gain (dB) at knob 0, and the extra at knob 1.
const FUZZ_MIN_DB: f32 = 20.0;
const FUZZ_RANGE_DB: f32 = 30.0;
/// Second (hard clipper) stage gain, dB: fuzz into fuzz.
const FUZZ2_DB: f32 = 16.0;
/// Rectified (octave-up) content mixed into the first fuzz stage.
const OCTAVE: f32 = 0.45;

/// The clean sits this far under the fuzz (dB), plus the clean amp's makeup
/// gain so it lands there.
const HONEY_LEVEL_DB: f32 = -5.0;
const CLEAN_DRIVE: f32 = 1.6;
const CLEAN_MAKEUP_DB: f32 = 23.0;

/// Reverb: wet level at full Reverb knob for each voice (the clean is
/// drenched, the fuzz a little drier so it stays a wall rather than mush),
/// room feedback range, damping and pre-delay (samples, 22 ms).
const HONEY_WET: f32 = 1.15;
const PSYCHO_WET: f32 = 0.8;
const ROOM_MIN: f32 = 0.80;
const ROOM_RANGE: f32 = 0.165;
const DAMP: f32 = 0.38;
const PREDELAY: f32 = 1056.0;

/// Feedback squall: loop delay (samples, ~ distance amp -> guitar), band-pass
/// Q, the centre it wanders around (Hz) and how far (octaves), the wander
/// rates (Hz), loop gain at full bloom, how far the string's own sound is
/// pushed down once the squall has taken it over, and bloom / die times (s).
const FB_DELAY: f32 = 263.0;
const FB_Q: f32 = 7.0;
const FB_FREQ: f32 = 2300.0;
const FB_WANDER: f32 = 0.65;
const FB_RATE_A: f32 = 0.11;
const FB_RATE_B: f32 = 0.29;
const FB_LOOP: f32 = 1.5;
const FB_DUCK: f32 = 0.6;
const FB_BLOOM_S: f32 = 1.2;
const FB_DIE_S: f32 = 0.25;

/// Noise gate on the fuzz (input envelope, linear, with hysteresis); it
/// closes slowly so tails and squall die naturally rather than chop.
const GATE_OPEN: f32 = 0.0010;
const GATE_CLOSE: f32 = 0.0004;

/// Output trim: unity Level puts the fuzz strums around -14 dB RMS.
const OUT_TRIM: f32 = 0.16;

// ---------------------------------------------------------------- reverb

/// Freeverb-style lowpass-feedback comb with a runtime length.
struct Comb<const N: usize> {
    buf: [f32; N],
    len: usize,
    i: usize,
    filt: f32,
}

impl<const N: usize> Comb<N> {
    const NEW: Self = Self { buf: [0.0; N], len: N, i: 0, filt: 0.0 };

    #[inline]
    fn process(&mut self, x: f32, fb: f32, damp: f32) -> f32 {
        let o = self.buf[self.i];
        self.filt = o + (self.filt - o) * damp;
        self.buf[self.i] = x + self.filt * fb;
        self.i += 1;
        if self.i >= self.len {
            self.i = 0;
        }
        o
    }
}

/// Schroeder allpass (feedback 0.5) with a runtime length.
struct Allpass<const N: usize> {
    buf: [f32; N],
    len: usize,
    i: usize,
}

impl<const N: usize> Allpass<N> {
    const NEW: Self = Self { buf: [0.0; N], len: N, i: 0 };

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let o = self.buf[self.i];
        self.buf[self.i] = x + o * 0.5;
        self.i += 1;
        if self.i >= self.len {
            self.i = 0;
        }
        o - x
    }
}

/// Freeverb tunings (44.1 kHz) scaled to 48 kHz; the right channel is spread.
const COMB_LEN: [usize; 8] = [1214, 1293, 1389, 1475, 1547, 1622, 1694, 1759];
const AP_LEN: [usize; 4] = [605, 480, 371, 245];
const SPREAD: usize = 25;

struct Reverb {
    pre: Delay<2048>,
    cl: [Comb<2048>; 8],
    cr: [Comb<2048>; 8],
    al: [Allpass<1024>; 4],
    ar: [Allpass<1024>; 4],
}

impl Reverb {
    const NEW: Self = Self {
        pre: Delay::NEW,
        cl: [Comb::NEW; 8],
        cr: [Comb::NEW; 8],
        al: [Allpass::NEW; 4],
        ar: [Allpass::NEW; 4],
    };

    fn init(&mut self) {
        for k in 0..8 {
            self.cl[k].len = COMB_LEN[k];
            self.cr[k].len = COMB_LEN[k] + SPREAD;
        }
        for k in 0..4 {
            self.al[k].len = AP_LEN[k];
            self.ar[k].len = AP_LEN[k] + SPREAD;
        }
    }

    /// Wet only, stereo, at roughly the input's level.
    #[inline]
    fn process(&mut self, x: f32, room: f32) -> (f32, f32) {
        self.pre.push(x);
        let x = self.pre.read(PREDELAY) * 0.015;
        let mut l = 0.0;
        let mut r = 0.0;
        for c in self.cl.iter_mut() {
            l += c.process(x, room, DAMP);
        }
        for c in self.cr.iter_mut() {
            r += c.process(x, room, DAMP);
        }
        for a in self.al.iter_mut() {
            l = a.process(l);
        }
        for a in self.ar.iter_mut() {
            r = a.process(r);
        }
        (l * 3.0, r * 3.0)
    }
}

// ---------------------------------------------------------------- state

struct Jm {
    ready: bool,
    error: bool,
    // knobs, 0..1
    fuzz: f32,
    reverb: f32,
    level: f32,
    loud: bool,
    feedback: bool,
    // smoothed values and targets
    g_fuzz: f32,
    g_fuzz_t: f32,
    out_g: f32,
    out_t: f32,
    rev_s: f32,
    mix: f32,
    fb_ramp: f32,
    // filters
    hp_in: Biquad,
    fz_hp: Biquad,
    fz_emph: OnePole,
    fz_inter_hp: Biquad,
    fz_inter_lp: Biquad,
    fz_scoop: Biquad,
    fz_bite: Biquad,
    fz_lp: Biquad,
    clean_hp: Biquad,
    clean_low: Biquad,
    clean_jangle: Biquad,
    cab: [Biquad; 6],
    fb_bp: Biquad,
    u1: f32,
    u2: f32,
    // dynamics
    env: f32,
    gate: f32,
    gate_open: bool,
    comp: f32,
    // feedback loop
    fb_line: Delay<512>,
    fb_ph_a: f32,
    fb_ph_b: f32,
    fb_count: u32,
    // reverb
    rev: Reverb,
    // coefficients
    c_param: f32,
    c_mix: f32,
    c_att: f32,
    c_rel: f32,
    c_gate_open: f32,
    c_gate_close: f32,
    c_comp: f32,
    c_fb_up: f32,
    c_fb_down: f32,
}

impl Jm {
    const NEW: Self = Self {
        ready: false,
        error: false,
        fuzz: 0.75,
        reverb: 0.55,
        level: 0.75,
        loud: true,
        feedback: false,
        g_fuzz: 1.0,
        g_fuzz_t: 1.0,
        out_g: 0.0,
        out_t: 0.0,
        rev_s: 0.55,
        mix: 1.0,
        fb_ramp: 0.0,
        hp_in: Biquad::NEW,
        fz_hp: Biquad::NEW,
        fz_emph: OnePole::NEW,
        fz_inter_hp: Biquad::NEW,
        fz_inter_lp: Biquad::NEW,
        fz_scoop: Biquad::NEW,
        fz_bite: Biquad::NEW,
        fz_lp: Biquad::NEW,
        clean_hp: Biquad::NEW,
        clean_low: Biquad::NEW,
        clean_jangle: Biquad::NEW,
        cab: [Biquad::NEW; 6],
        fb_bp: Biquad::NEW,
        u1: 0.0,
        u2: 0.0,
        env: 0.0,
        gate: 0.0,
        gate_open: false,
        comp: 1.0,
        fb_line: Delay::NEW,
        fb_ph_a: 0.0,
        fb_ph_b: 0.4,
        fb_count: 0,
        rev: Reverb::NEW,
        c_param: 1.0,
        c_mix: 1.0,
        c_att: 1.0,
        c_rel: 1.0,
        c_gate_open: 1.0,
        c_gate_close: 1.0,
        c_comp: 1.0,
        c_fb_up: 1.0,
        c_fb_down: 1.0,
    };

    fn init(&mut self) {
        self.hp_in.highpass(80.0, 0.707);
        // Psychocandy: thin and trebly into the fuzz, nothing below ~140 Hz.
        self.fz_hp.highpass(140.0, 0.707);
        self.fz_emph.set(900.0);
        self.fz_inter_hp.highpass(160.0, 0.707); // also strips the octave's DC
        self.fz_inter_lp.lowpass(6500.0, 0.707);
        // Scooped body, a fierce upper-mid bite, fizz left on top.
        self.fz_scoop.peak(650.0, 0.7, -5.0);
        self.fz_bite.peak(2800.0, 1.0, 6.0);
        self.fz_lp.lowpass(8500.0, 0.707);
        // Honey: bright, jangly clean (a hollow-body through a clean combo).
        self.clean_hp.highpass(100.0, 0.707);
        self.clean_low.peak(250.0, 0.8, -2.5);
        self.clean_jangle.peak(3600.0, 0.8, 5.0);
        // Small open-back combo: less bottom and more top than a 4x12.
        self.cab[0].highpass(85.0, 0.707);
        self.cab[1].peak(120.0, 1.0, 2.0);
        self.cab[2].peak(2500.0, 1.2, 2.0);
        self.cab[3].peak(5000.0, 2.0, -2.0);
        self.cab[4].lowpass(7000.0, 0.7);
        self.cab[5].lowpass(10_000.0, 0.6);
        self.fb_bp.bandpass(FB_FREQ, FB_Q);
        self.rev.init();
        self.c_param = coef(0.02);
        self.c_mix = coef(0.012);
        self.c_att = coef(0.001);
        self.c_rel = coef(0.08);
        self.c_gate_open = coef(0.002);
        self.c_gate_close = coef(0.12);
        self.c_comp = coef(0.05);
        self.c_fb_up = coef(FB_BLOOM_S / 3.0); // ~95% bloomed after FB_BLOOM_S
        self.c_fb_down = coef(FB_DIE_S);
        self.retarget();
        self.g_fuzz = self.g_fuzz_t;
        self.out_g = self.out_t;
        self.rev_s = self.reverb;
        self.mix = if self.loud { 1.0 } else { 0.0 };
        self.ready = true;
    }

    fn retarget(&mut self) {
        self.g_fuzz_t = db(FUZZ_MIN_DB + FUZZ_RANGE_DB * self.fuzz);
        // Square law: 0 at minimum, unity at 3 o'clock, +5 dB at maximum.
        let v = self.level / 0.75;
        self.out_t = v * v * OUT_TRIM;
    }

    /// Psychocandy voice: fuzz into fuzz -> scooped, biting EQ (mono).
    #[inline]
    fn fuzz(&mut self, x: f32) -> f32 {
        let x = self.fz_hp.process(x);
        let x = x + 2.0 * (x - self.fz_emph.process(x)); // ~+10 dB above 900 Hz
        // Stage 1, Super-Fuzz style: transistor saturation plus its full-wave
        // rectified copy (an octave up), with cheap 2x oversampling.
        let u = x * self.g_fuzz;
        let y = 0.5 * (octave(0.5 * (u + self.u1)) + octave(u));
        self.u1 = u;
        let y = self.fz_inter_lp.process(self.fz_inter_hp.process(y));
        // Stage 2: slam it into a hard clipper.
        let u = y * db(FUZZ2_DB);
        let y = 0.5 * (hard(0.5 * (u + self.u2)) + hard(u));
        self.u2 = u;
        let y = self.fz_bite.process(self.fz_scoop.process(y));
        self.fz_lp.process(y)
    }

    /// Honey voice: clean amp with gentle compression and jangle (mono).
    #[inline]
    fn clean(&mut self, x: f32) -> f32 {
        let x = self.clean_hp.process(x);
        let a = x.abs() * CLEAN_DRIVE;
        let target = if a > 0.5 { 0.5 / a } else { 1.0 };
        self.comp += (target - self.comp) * if target < self.comp { 0.05 } else { self.c_comp };
        let y = tanh(x * CLEAN_DRIVE * self.comp) * 0.7;
        self.clean_jangle.process(self.clean_low.process(y))
    }

    #[inline]
    fn tick(&mut self, x: f32) -> (f32, f32) {
        self.g_fuzz += (self.g_fuzz_t - self.g_fuzz) * self.c_param;
        self.out_g += (self.out_t - self.out_g) * self.c_param;
        self.rev_s += (self.reverb - self.rev_s) * self.c_param;
        let mix_t = if self.loud { 1.0 } else { 0.0 };
        self.mix += (mix_t - self.mix) * self.c_mix;

        let x = self.hp_in.process(x);
        let a = x.abs();
        self.env += (a - self.env) * if a > self.env { self.c_att } else { self.c_rel };
        if self.env > GATE_OPEN {
            self.gate_open = true;
        } else if self.env < GATE_CLOSE {
            self.gate_open = false;
        }
        let (gt, gc) = if self.gate_open { (1.0, self.c_gate_open) } else { (0.0, self.c_gate_close) };
        self.gate += (gt - self.gate) * gc;

        // Squall: its band-pass wanders over ~1.5 to 3.6 kHz on two slow
        // sines, so the screech changes pitch by itself. Retuned every 32
        // samples. Injection is normalised by the first fuzz stage's gain so
        // the bloom is the same at any Fuzz setting. As it blooms the squall
        // takes the string over (the amp is driving it), so the picked sound
        // is ducked under it; otherwise the much louder string would hog the
        // clippers and smother the feedback.
        self.fb_ph_a += FB_RATE_A / SR;
        self.fb_ph_b += FB_RATE_B / SR;
        if self.fb_ph_a >= 1.0 {
            self.fb_ph_a -= 1.0;
        }
        if self.fb_ph_b >= 1.0 {
            self.fb_ph_b -= 1.0;
        }
        self.fb_count = self.fb_count.wrapping_add(1);
        if self.fb_count & 31 == 0 {
            let w = 0.6 * sin(TAU * self.fb_ph_a) + 0.4 * sin(TAU * self.fb_ph_b);
            self.fb_bp.bandpass(FB_FREQ * exp2(FB_WANDER * w), FB_Q);
        }
        let ft = if self.feedback && self.loud && self.gate_open { 1.0 } else { 0.0 };
        self.fb_ramp += (ft - self.fb_ramp) * if ft > self.fb_ramp { self.c_fb_up } else { self.c_fb_down };
        let fb = self.fb_line.read(FB_DELAY) * self.fb_ramp * FB_LOOP / self.g_fuzz;
        let duck = 1.0 - FB_DUCK * self.fb_ramp;

        // Both voices always run, so switching is a click-free crossfade.
        let d = self.fuzz(x * duck + fb) * self.gate;
        self.fb_line.push(self.fb_bp.process(d));
        let c = self.clean(x) * db(HONEY_LEVEL_DB + CLEAN_MAKEUP_DB);
        let m = self.mix;
        let mut y = c + (d - c) * m;
        for b in self.cab.iter_mut() {
            y = b.process(y);
        }

        // Shared reverb, so its tail carries across a switch.
        let room = ROOM_MIN + ROOM_RANGE * self.rev_s;
        let (wl, wr) = self.rev.process(y, room);
        let wet = self.rev_s * (HONEY_WET + (PSYCHO_WET - HONEY_WET) * m);
        (limit((y + wet * wl) * self.out_g), limit((y + wet * wr) * self.out_g))
    }

    fn led(&self) -> Color {
        if self.error {
            return Color::Red;
        }
        match (self.loud, self.feedback) {
            (false, false) => Color::LightYellow,
            (false, true) => Color::Beige,
            (true, false) => Color::Magenta,
            (true, true) => Color::Red,
        }
    }
}

/// Fuzz stage 1: saturation plus its full-wave rectified copy (octave up).
#[inline]
fn octave(u: f32) -> f32 {
    let a = tanh(u);
    a + OCTAVE * a.abs()
}

/// Hard clipper: cubic knee, flat beyond |u| = 1.5.
#[inline]
fn hard(u: f32) -> f32 {
    let c = (u * (1.0 / 1.5)).clamp(-1.0, 1.0);
    1.5 * c - 0.5 * c * c * c
}

/// Output soft limiter, never reaching ±1.
#[inline]
fn limit(x: f32) -> f32 {
    tanh(x) * 0.98
}

/// All patch state. The firmware calls every entry point from the audio
/// thread (SDK: "no need to synchronize").
struct Global(UnsafeCell<Jm>);
unsafe impl Sync for Global {}
static STATE: Global = Global(UnsafeCell::new(Jm::NEW));

#[allow(clippy::mut_from_ref)]
fn state() -> &'static mut Jm {
    // SAFETY: single-threaded access from the firmware's audio thread.
    unsafe { &mut *STATE.0.get() }
}

// ---------------------------------------------------------------- entry points

const PARAM_FUZZ: i32 = 0;
const PARAM_REVERB: i32 = 1;
const PARAM_LEVEL: i32 = 2;

unsafe extern "C" fn patch_init(env: *const PatchEnv) {
    if env.is_null() || unsafe { (*env).abi_version } != PATCH_ABI_VERSION as u32 {
        return;
    }
    state().init();
}

unsafe extern "C" fn update_buffers(_: *const PatchEnv, left: *mut f32, right: *mut f32, samples: i32) -> f32 {
    let Ok(n) = usize::try_from(samples) else { return 0.0 };
    if left.is_null() || right.is_null() || n == 0 {
        return 0.0;
    }
    // SAFETY: the firmware passes two buffers of `samples` floats.
    let (l, r) = unsafe { (core::slice::from_raw_parts_mut(left, n), core::slice::from_raw_parts_mut(right, n)) };
    let s = state();
    if !s.ready {
        return 0.0;
    }
    for (a, b) in l.iter_mut().zip(r.iter_mut()) {
        let (ol, or) = s.tick(*a);
        *a = ol;
        *b = or;
    }
    0.0
}

unsafe extern "C" fn set_buffer(_: *const PatchEnv, buffer: *mut f32) {
    // All state is static; the working buffer isn't needed.
    if buffer.is_null() {
        state().error = true;
    }
}

unsafe extern "C" fn get_buffer_size(_: *const PatchEnv) -> i32 {
    WORKING_BUFFER_SIZE
}

unsafe extern "C" fn get_param_min(_: *const PatchEnv, _: i32) -> f32 {
    0.0
}

unsafe extern "C" fn get_param_max(_: *const PatchEnv, _: i32) -> f32 {
    1.0
}

unsafe extern "C" fn get_param_default(_: *const PatchEnv, idx: i32) -> f32 {
    match idx {
        PARAM_FUZZ => 0.75,
        PARAM_REVERB => 0.55,
        _ => 0.75,
    }
}

unsafe extern "C" fn is_param_enabled(_: *const PatchEnv, idx: i32, source_id: i32) -> u8 {
    // Knobs only (source 0), like the SDK wrapper.
    (source_id == 0 && (0..PARAM_COUNT).contains(&idx)) as u8
}

unsafe fn write_c_str(s: &str, out: *mut u8, len: usize) {
    if out.is_null() || len == 0 {
        return;
    }
    let n = s.len().min(len - 1);
    // SAFETY: the firmware provides `len` writable bytes at `out`.
    unsafe {
        core::ptr::copy_nonoverlapping(s.as_ptr(), out, n);
        *out.add(n) = 0;
    }
}

unsafe extern "C" fn get_param_name(_: *const PatchEnv, idx: i32, out: *mut u8, len: usize) {
    let name = match idx {
        PARAM_FUZZ => "Fuzz",
        PARAM_REVERB => "Reverb",
        PARAM_LEVEL => "Level",
        _ => "",
    };
    unsafe { write_c_str(name, out, len) }
}

unsafe extern "C" fn get_param_unit(_: *const PatchEnv, _: i32, out: *mut u8, len: usize) {
    unsafe { write_c_str("", out, len) }
}

unsafe extern "C" fn set_param(_: *const PatchEnv, idx: i32, value: f32) {
    let s = state();
    let v = value.clamp(0.0, 1.0);
    match idx {
        PARAM_FUZZ => s.fuzz = v,
        PARAM_REVERB => s.reverb = v,
        PARAM_LEVEL => s.level = v,
        _ => return,
    }
    if s.ready {
        s.retarget();
    }
}

unsafe extern "C" fn special_action(_: *const PatchEnv, idx: i32) {
    let s = state();
    match idx {
        ACTION_FOOTSWITCH_PRESS => s.loud = !s.loud,
        ACTION_FOOTSWITCH_HOLD => s.feedback = !s.feedback,
        _ => return,
    }
    if s.ready {
        s.retarget();
    }
}

unsafe extern "C" fn get_state_idx(_: *const PatchEnv) -> i32 {
    state().led() as i32
}

// ---------------------------------------------------------------- header

#[cfg(target_os = "none")]
unsafe extern "C" {
    static __patch_bss_start: u8;
    static patch_image_size: u8;
    static patch_bss_size: u8;
}

#[cfg(target_os = "none")]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".patch_header")]
#[used]
pub static patch_header: PatchHeader = PatchHeader {
    magic: PATCH_MAGIC,
    abi_version: PATCH_ABI_VERSION,
    flags: PATCH_FLAG_NONE,
    init: patch_init,
    agent_update_buffers: update_buffers,
    agent_set_buffer: set_buffer,
    agent_get_buffer_size: get_buffer_size,
    agent_get_param_min: get_param_min,
    agent_get_param_max: get_param_max,
    agent_get_param_default: get_param_default,
    agent_is_param_enabled: is_param_enabled,
    agent_get_param_name: get_param_name,
    agent_get_param_unit: get_param_unit,
    agent_set_param: set_param,
    agent_special_action: special_action,
    agent_get_state_idx: get_state_idx,
    image_size: &raw const patch_image_size,
    bss_begin: &raw const __patch_bss_start,
    bss_size: &raw const patch_bss_size,
    reserved: [0; 16],
};

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(not(target_os = "none"))]
#[allow(dead_code)] // each demo uses only some of the playing techniques
mod guitar;
#[cfg(not(target_os = "none"))]
mod host;

#[cfg(not(target_os = "none"))]
fn main() {
    host::main();
}
