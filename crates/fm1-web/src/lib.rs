//! The FM-1 emulator for the browser. One WebAssembly module, two users:
//!
//! * the AudioWorklet runs the machine (`fm1_*`): boot from a firmware image, render audio
//!   frames on demand (the emulator runs until the I2S DMA has produced them, so the audio
//!   device paces it), take key / encoder / pot input, expose the LCD and LED state;
//! * the page draws the front panel (`panel_*`, the same code as the native window) and
//!   hit-tests the pointer.
//!
//! Plain `extern "C"` exports and pointers into linear memory: no bindings generator needed.
//!
//! Hot code runs compiled to WebAssembly once the host turns it on (`fm1_jit`): the module
//! imports `env.fm1_jit_compile`, which compiles a block's module and installs it in the exported
//! function table (web/jit.js; a host without it passes a stub that returns 0).

use fm1_core::{Bus, Cpu};
use fm1_soc::{machine, Action, Soc};
use std::collections::VecDeque;

#[path = "../../fm1-live/src/panel.rs"]
mod panel;
#[path = "../../fm1-live/src/panel_smk.rs"]
mod panel_smk;

const RAM_BASE: u32 = 0x01c0_0000;
const RAM_SIZE: usize = 1 << 20;

struct Machine {
    cpu: Cpu,
    /// the second core once the firmware releases it (fm1_soc::machine)
    cpu1: Option<Cpu>,
    soc: Soc,
    pending_tick: bool,
    /// interleaved L/R i16 not yet handed out
    audio: VecDeque<i16>,
    halted: bool,
}

impl Machine {
    /// `stock`: a package's flash (its chip and boot parameters come along) and board.
    fn new(image: Vec<u8>, entry: u32, mhz: u32, stock: Option<(&[u8], fm1_soc::Board)>) -> Self {
        let mut soc = Soc::new(RAM_SIZE, image);
        soc.periph.hz = mhz as u64 * 1_000_000;
        soc.periph.skip_idle = true;
        let mut cpu = Cpu::new(entry);
        // (cpu1 follows cpu0's setting: run_dual)
        cpu.jit_on = unsafe { JIT };
        cpu.user = true; // the SPL hands cpu0 over in user mode (fm1-core's Cpu::user)
        let params = RAM_BASE + 0x7fc00;
        soc.write32(params, params + 0x40);
        cpu.regs[0] = params;
        if let Some((f, board)) = stock {
            cpu.regs[0] = fm1_soc::stock_boot(&mut soc, f, params, board);
        }
        Machine { cpu, cpu1: None, soc, pending_tick: false, audio: VecDeque::new(), halted: false }
    }

    /// fm1-live's loop: cpu0's blocks (interrupts exact at block granularity), cpu1 beside.
    fn run(&mut self, n: u32) {
        let jit = self.cpu.jit_on;
        if let Some(c1) = self.cpu1.as_mut() { c1.profile = self.cpu.profile; }
        let Machine { cpu, cpu1, soc, pending_tick, .. } = self;
        if machine::run_dual(cpu1, soc, n, RAM_SIZE, jit, |soc, k| machine::run_blocks(cpu, soc, k, pending_tick)).is_err() {
            self.halted = true;
        }
    }
}

static mut M: Option<Machine> = None;
static mut JIT: bool = false;
static mut JIT_PER_RENDER: u32 = 0;
static mut OUT_L: [f32; 1024] = [0.0; 1024];
static mut OUT_R: [f32; 1024] = [0.0; 1024];
static mut LEDS: [f32; 41] = [0.0; 41];
static mut PADS: [u32; 16] = [0; 16];
static mut INSNS: u64 = 0;

#[allow(static_mut_refs)]
fn machine() -> Option<&'static mut Machine> {
    // SAFETY: wasm32 is single-threaded per instance; every export runs to completion.
    unsafe { M.as_mut() }
}

/// Bytes for JS to copy a firmware image into (never freed: one image per instance).
#[no_mangle]
pub extern "C" fn fm1_alloc(len: u32) -> *mut u8 {
    let mut v = vec![0u8; len as usize];
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// Boot the image at `ptr` (from fm1_alloc). `entry` 0x02000120 for Felucca.
#[no_mangle]
pub extern "C" fn fm1_boot(ptr: *mut u8, len: u32, entry: u32, mhz: u32) {
    // SAFETY: ptr/len come from fm1_alloc and JS filled them.
    let image = unsafe { Vec::from_raw_parts(ptr, len as usize, len as usize) };
    unsafe { M = Some(Machine::new(image, entry, mhz, None)) };
}

/// Boot M-VAVE's stock firmware from its whole update package (.fwsc) at `ptr`, with a DX7
/// 32-voice bank (.syx) for patch slots 1-32 at `syx` when `syx_len` is not 0 (both from
/// fm1_alloc). Returns 0, or 1 when the package does not unpack (the machine is not booted).
#[no_mangle]
pub extern "C" fn fm1_boot_fwsc(ptr: *mut u8, len: u32, syx: *mut u8, syx_len: u32, mhz: u32) -> u32 {
    // SAFETY: as fm1_boot.
    let pkg = unsafe { Vec::from_raw_parts(ptr, len as usize, len as usize) };
    let bank = (syx_len != 0).then(|| unsafe { Vec::from_raw_parts(syx, syx_len as usize, syx_len as usize) });
    match fm1_soc::stock_from_package(&pkg, bank.as_deref()) {
        Ok(st) => {
            unsafe { M = Some(Machine::new(st.xip, st.entry, mhz, Some((&st.flash, st.board)))) };
            0
        }
        Err(_) => 1,
    }
}

/// Compile hot code to WebAssembly (`on` = 1; the host's `env.fm1_jit_compile` does the
/// compiling), from the next boot on and in the running machine. `per_render`: blocks that may be
/// compiled in one `fm1_render` (0: no limit), so compiling never holds up an audio quantum long.
#[no_mangle]
pub extern "C" fn fm1_jit(on: u32, per_render: u32) {
    unsafe {
        JIT = on != 0;
        JIT_PER_RENDER = per_render;
    }
    if let Some(m) = machine() {
        m.cpu.jit_on = on != 0;
        if let Some(c1) = m.cpu1.as_mut() {
            c1.jit_on = on != 0;
        }
    }
}

/// Optional profiling, disabled during throughput measurements. Turn it on before the first
/// fm1_render: the JIT, made then, compiles helper calls that count themselves only if it is.
#[no_mangle]
pub extern "C" fn fm1_profile(on: u32) {
    if let Some(m) = machine() { m.cpu.profile = on != 0; }
}

static mut PROFILE_JSON: Vec<u8> = Vec::new();
/// NUL-terminated JSON valid until the next report call.
#[no_mangle]
#[allow(static_mut_refs)]
pub extern "C" fn fm1_profile_report() -> *const u8 {
    let report = machine().map(|m| format!("[{},{}]", m.cpu.jit_profile_json(),
        m.cpu1.as_ref().map_or_else(|| "null".to_owned(), |c| c.jit_profile_json()))).unwrap_or_else(|| "[]".to_owned());
    unsafe {
        PROFILE_JSON = report.into_bytes();
        PROFILE_JSON.push(0);
        PROFILE_JSON.as_ptr()
    }
}

/// Install the same timestamped input script used by the native benchmark.
#[no_mangle]
pub extern "C" fn fm1_bench_script(ptr: *mut u8, len: u32) -> u32 {
    // SAFETY: owned allocation returned by fm1_alloc, consumed exactly once.
    let bytes = unsafe { Vec::from_raw_parts(ptr, len as usize, len as usize) };
    let Ok(script) = std::str::from_utf8(&bytes) else { return 1 };
    let Some(m) = machine() else { return 1 };
    m.soc.periph.script.clear();
    fm1_soc::script::queue(&mut m.soc, script);
    0
}

/// Render `frames` (<= 1024) stereo frames into the out buffers, running the machine until the
/// I2S DMA has produced them (at most `max_insns`). Returns the frames that are real audio.
#[no_mangle]
pub extern "C" fn fm1_render(frames: u32, max_insns: u32) -> u32 {
    let Some(m) = machine() else { return 0 };
    #[cfg(target_arch = "wasm32")]
    {
        let per = unsafe { JIT_PER_RENDER };
        fm1_core::jit_wasm::COMPILE_BUDGET.store(if per == 0 { u32::MAX } else { per }, std::sync::atomic::Ordering::Relaxed);
    }
    let frames = frames.min(1024) as usize;
    let mut spent = 0u32;
    while m.audio.len() < 2 * frames && spent < max_insns && !m.halted {
        m.run(20_000);
        spent += 20_000;
        m.audio.extend(m.soc.periph.audio.drain(..));
    }
    unsafe { INSNS += spent as u64 };
    let have = (m.audio.len() / 2).min(frames);
    #[allow(static_mut_refs)]
    unsafe {
        for i in 0..frames {
            if i < have {
                OUT_L[i] = m.audio.pop_front().unwrap_or(0) as f32 / 32768.0;
                OUT_R[i] = m.audio.pop_front().unwrap_or(0) as f32 / 32768.0;
            } else {
                OUT_L[i] = 0.0;
                OUT_R[i] = 0.0;
            }
        }
    }
    have as u32
}

#[no_mangle]
pub extern "C" fn fm1_out_l() -> *const f32 {
    #[allow(static_mut_refs)]
    unsafe { OUT_L.as_ptr() }
}

#[no_mangle]
pub extern "C" fn fm1_out_r() -> *const f32 {
    #[allow(static_mut_refs)]
    unsafe { OUT_R.as_ptr() }
}

/// The 240x240 RGB565 framebuffer.
#[no_mangle]
pub extern "C" fn fm1_lcd() -> *const u16 {
    machine().map_or(std::ptr::null(), |m| m.soc.lcd.fb.as_ptr())
}

/// Framebuffer revision, including partial window updates.
#[no_mangle]
pub extern "C" fn fm1_lcd_revision() -> u32 {
    machine().map_or(0, |m| m.soc.lcd.revision)
}

/// The LEDs' on-fractions since the last call (41 floats; SMK-37: its 19 button LEDs first).
#[no_mangle]
pub extern "C" fn fm1_leds() -> *const f32 {
    #[allow(static_mut_refs)]
    unsafe {
        if let Some(m) = machine() {
            if m.soc.board == fm1_soc::Board::Smk37 {
                LEDS = [0.0; 41];
                LEDS[..fm1_soc::smk::NLED].copy_from_slice(&m.soc.periph.smk.led_take());
            } else {
                LEDS = m.soc.periph.led_take();
            }
        }
        LEDS.as_ptr()
    }
}

/// The board the running package is for: 0 the FM-1, 1 the SMK-37 (fm1_soc::smk).
#[no_mangle]
pub extern "C" fn fm1_board() -> u32 {
    machine().map_or(0, |m| (m.soc.board == fm1_soc::Board::Smk37) as u32)
}

/// An analog input: on the SMK-37 `idx` is fm1_soc::smk's ANALOG_* (pads 0..15, faders 16..19,
/// the pitch and mod wheels 20, 21, the pedal 22); on the FM-1 an ADC channel.
#[no_mangle]
pub extern "C" fn fm1_analog(idx: u32, value: u32) {
    if let Some(m) = machine() {
        m.soc.periph.apply(&Action::Adc { ch: idx as u8, value: value.min(1023) as u16 });
    }
}

/// The SMK-37's key velocity: microseconds between a key's two contacts (less is harder; the
/// stock curve gives the most below ~4 ms, the least above ~100 ms).
#[no_mangle]
pub extern "C" fn fm1_velocity(us: u32) {
    if let Some(m) = machine() {
        m.soc.periph.smk_vel = (m.soc.periph.hz as u128 * us as u128 / 1_000_000) as u64;
    }
}

/// The SMK-37's pad colours (16 x 0xRRGGBB) from its last LED frame.
#[no_mangle]
pub extern "C" fn fm1_pads() -> *const u32 {
    #[allow(static_mut_refs)]
    unsafe {
        if let Some(m) = machine() {
            PADS = m.soc.periph.smk.pads;
        }
        PADS.as_ptr()
    }
}

/// A key or button (matrix id 0..40; SMK-37: keys 0..36, buttons 37..54) down / up.
#[no_mangle]
pub extern "C" fn fm1_key(id: u32, down: u32) {
    if let Some(m) = machine() {
        m.soc.periph.apply(&Action::Key { id: id as u8, down: down != 0 });
    }
}

/// Turn encoder `enc` (matrix encoder 0..6) by `clicks` (+ = clockwise).
#[no_mangle]
pub extern "C" fn fm1_enc(enc: u32, clicks: i32) {
    if let Some(m) = machine() {
        m.soc.periph.apply(&Action::Enc { enc: enc as u8, clicks });
    }
}

/// One MIDI byte into the MIDI IN jack (UART1, 31250 baud: bytes queue up a byte time apart).
#[no_mangle]
pub extern "C" fn fm1_midi(b: u32) {
    if let Some(m) = machine() {
        m.soc.periph.apply(&Action::Midi { b: b as u8 });
    }
}

/// The MASTER pot, 0..1023.
#[no_mangle]
pub extern "C" fn fm1_pot(v: u32) {
    if let Some(m) = machine() {
        m.soc.periph.apply(&Action::Adc { ch: 4, value: v.min(1023) as u16 });
    }
}

/// Emulated milliseconds since boot.
#[no_mangle]
pub extern "C" fn fm1_ms() -> u32 {
    machine().map_or(0, |m| m.soc.periph.ms() as u32)
}

/// Instructions executed so far (for a speed readout), in millions.
#[no_mangle]
pub extern "C" fn fm1_minsns() -> u32 {
    unsafe { (INSNS / 1_000_000) as u32 }
}

#[no_mangle]
pub extern "C" fn fm1_halted() -> u32 {
    machine().map_or(1, |m| m.halted as u32)
}

// ---------------------------------------------------------------- the front panel (page side)

static mut PANEL_BASE: Vec<u32> = Vec::new();
static mut PANEL_BUF: Vec<u32> = Vec::new();
static mut PANEL_LCD: Vec<u16> = Vec::new();
static mut PANEL_LEDS: [f32; 41] = [0.0; 41];
static mut PANEL_DOWN: [bool; 64] = [false; 64];
static mut PANEL_ENC: [i32; 8] = [0; 8];
static mut PANEL_TURN: [u32; 9] = [0; 9];
static mut PANEL_POT: u16 = 900;
/// Which panel the page draws: 0 the FM-1's, 1 the SMK-37's.
static mut PANEL_BOARD: u32 = 0;
/// SMK-37: the analog inputs as set (fm1_soc::smk's ANALOG_* indices) and the pads' colours
static mut PANEL_ANALOG: [u16; 23] = [0; 23];
static mut PANEL_PADS: [u32; 16] = [0; 16];
/// how far down the key `panel_hit` found was hit (0 top .. 1 bottom)
static mut PANEL_HIT_Y: f32 = 0.0;

/// Draw the FM-1's panel (0) or the SMK-37's (1) from now on.
#[no_mangle]
#[allow(static_mut_refs)]
pub extern "C" fn panel_board(b: u32) {
    unsafe {
        if PANEL_BOARD != b {
            PANEL_BOARD = b;
            PANEL_BASE = Vec::new();
            if b == 1 {
                PANEL_ANALOG[fm1_soc::smk::ANALOG_PITCH as usize] = fm1_soc::smk::PITCH_REST;
                PANEL_ANALOG[fm1_soc::smk::ANALOG_MOD as usize] = fm1_soc::smk::MOD_REST;
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn panel_width() -> u32 {
    if unsafe { PANEL_BOARD } == 1 { panel_smk::W as u32 } else { panel::W as u32 }
}

#[no_mangle]
pub extern "C" fn panel_height() -> u32 {
    if unsafe { PANEL_BOARD } == 1 { panel_smk::H as u32 } else { panel::H as u32 }
}

/// Where the page writes the LCD frame (240*240 RGB565) before `panel_draw`.
#[no_mangle]
#[allow(static_mut_refs)]
pub extern "C" fn panel_lcd() -> *mut u16 {
    unsafe {
        if PANEL_LCD.is_empty() {
            PANEL_LCD = vec![0; 240 * 240];
        }
        PANEL_LCD.as_mut_ptr()
    }
}

/// Where the page writes the 41 LED levels before `panel_draw`.
#[no_mangle]
#[allow(static_mut_refs)]
pub extern "C" fn panel_leds() -> *mut f32 {
    unsafe { PANEL_LEDS.as_mut_ptr() }
}

/// UI state for the drawing: a key held, an encoder's clicks, the pot, a knob's glow.
#[no_mangle]
pub extern "C" fn panel_set(kind: u32, i: u32, v: i32) {
    unsafe {
        match kind {
            0 if i < 64 => PANEL_DOWN[i as usize] = v != 0,
            1 if i < 8 => PANEL_ENC[i as usize] = v,
            2 => PANEL_POT = v.clamp(0, 1023) as u16,
            3 if i < 9 => PANEL_TURN[i as usize] = v.max(0) as u32,
            // SMK-37: an analog input as set (ANALOG_* index), a pad's colour
            4 if i < 23 => PANEL_ANALOG[i as usize] = v.clamp(0, 1023) as u16,
            5 if i < 16 => PANEL_PADS[i as usize] = v as u32,
            _ => {}
        }
    }
}

/// Draw the panel; returns the RGBA bytes (W*H*4) for an ImageData.
#[no_mangle]
#[allow(static_mut_refs)]
pub extern "C" fn panel_draw() -> *const u8 {
    unsafe {
        let smk = PANEL_BOARD == 1;
        if PANEL_BASE.is_empty() {
            PANEL_BASE = if smk { panel_smk::base() } else { panel::base() };
            PANEL_BUF = vec![0; PANEL_BASE.len()];
        }
        if PANEL_LCD.is_empty() {
            PANEL_LCD = vec![0; 240 * 240];
        }
        if smk {
            let a = &PANEL_ANALOG;
            let st = panel_smk::State {
                lcd: &PANEL_LCD,
                enc_clicks: &PANEL_ENC,
                turning: &[PANEL_TURN[0], PANEL_TURN[1], PANEL_TURN[2], PANEL_TURN[3], PANEL_TURN[4], PANEL_TURN[5], PANEL_TURN[6], PANEL_TURN[7]],
                down: &PANEL_DOWN,
                leds: &std::array::from_fn(|k| PANEL_LEDS[k]),
                pads: &PANEL_PADS,
                pad_force: &std::array::from_fn(|p| a[p]),
                faders: &std::array::from_fn(|f| a[16 + f]),
                pitch: a[fm1_soc::smk::ANALOG_PITCH as usize],
                modw: a[fm1_soc::smk::ANALOG_MOD as usize],
            };
            panel_smk::draw(&mut PANEL_BUF, &PANEL_BASE, &st);
        } else {
            let st = panel::State {
                lcd: &PANEL_LCD,
                enc_clicks: &std::array::from_fn(|i| PANEL_ENC[i]),
                pot: PANEL_POT,
                down: &std::array::from_fn(|i| PANEL_DOWN[i]),
                leds: &PANEL_LEDS,
                turning: &std::array::from_fn(|i| PANEL_TURN[i]),
            };
            panel::draw(&mut PANEL_BUF, &PANEL_BASE, &st);
        }
        // 0RGB -> RGBA bytes in place (little endian: R G B A)
        for p in PANEL_BUF.iter_mut() {
            let (r, g, b) = ((*p >> 16) & 255, (*p >> 8) & 255, *p & 255);
            *p = r | g << 8 | b << 16 | 0xff00_0000;
        }
        PANEL_BUF.as_ptr() as *const u8
    }
}

/// What is under the pointer: -1 nothing, 0..6 an encoder (index into the panel's list), 7 the
/// pot, 100 + id a button or key (matrix id). SMK-37: 0..7 K1..K8, 100 + id a key (0..36, how
/// far down in `panel_hit_y`) or a button (37..54), 200 + p a pad, 300 + f a fader, 400 / 401 the
/// pitch / mod wheel.
#[no_mangle]
pub extern "C" fn panel_hit(x: f32, y: f32) -> i32 {
    if unsafe { PANEL_BOARD } == 1 {
        return match panel_smk::hit(x, y) {
            Some(panel_smk::Ctl::Enc(i)) => i as i32,
            Some(panel_smk::Ctl::Btn(id)) => 100 + id as i32,
            Some(panel_smk::Ctl::Key(id, f)) => {
                unsafe { PANEL_HIT_Y = f };
                100 + id as i32
            }
            Some(panel_smk::Ctl::Pad(p)) => 200 + p as i32,
            Some(panel_smk::Ctl::Fader(f)) => 300 + f as i32,
            Some(panel_smk::Ctl::Pitch) => 400,
            Some(panel_smk::Ctl::Mod) => 401,
            None => -1,
        };
    }
    match panel::hit(x, y) {
        Some(panel::Ctl::Enc(i)) => i as i32,
        Some(panel::Ctl::Pot) => 7,
        Some(panel::Ctl::Btn(id)) | Some(panel::Ctl::Key(id)) => 100 + id as i32,
        None => -1,
    }
}

/// The matrix encoder of panel encoder `i` (SELECT, ALGO, PRESET, K1..K4 order of the panel;
/// SMK-37: K1..K8 are encoders 0..7).
#[no_mangle]
pub extern "C" fn panel_enc_id(i: u32) -> u32 {
    if unsafe { PANEL_BOARD } == 1 {
        return i;
    }
    panel::ENCS.get(i as usize).map_or(0, |e| e.1 as u32)
}

/// How far down the last key `panel_hit` found was hit, 0 (top) .. 1 (bottom).
#[no_mangle]
pub extern "C" fn panel_hit_y() -> f32 {
    unsafe { PANEL_HIT_Y }
}

/// SMK-37: the reading for a fader (0..3) or a wheel (4 pitch, 5 mod) at panel y.
#[no_mangle]
pub extern "C" fn panel_slide(i: u32, y: f32) -> u32 {
    match i {
        0..=3 => panel_smk::fader_value(i as usize, y) as u32,
        4 => panel_smk::wheel_value(true, y) as u32,
        _ => panel_smk::wheel_value(false, y) as u32,
    }
}
