//! The SMK-37's front panel, after fm1-to-smk37's panel map (assets/smk37-panel.png): the pitch
//! and mod wheels and the six small buttons on the left, the screen (2x here, small on the unit),
//! K1..K8 over two rows of eight RGB pads, the four faders and twelve buttons on the right, and
//! the 37 keys C3..C6 across the bottom. Ids are fm1_soc::smk's: keys 0..36, buttons 37..54.

use crate::panel::{canvas, knob, mix, rrect, text, AMBER, BTN_BOT, BTN_INK, BTN_TOP, ROOM, SILK, TRAY};

pub const W: usize = 1920;
pub const H: usize = 900;
pub const LCD_X: usize = 288;
pub const LCD_Y: usize = 48;
const NKEY: usize = 37;
const BTN0: u8 = 37;

/// What a point on the panel is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ctl {
    /// An encoder K1..K8.
    Enc(usize),
    /// A pad (hardware index 0..15: the top row 0..7).
    Pad(usize),
    /// A fader F1..F4.
    Fader(usize),
    Pitch,
    Mod,
    /// A button (id 37..54) or a key (0..36); `y` how far down the key, 0..1.
    Btn(u8),
    Key(u8, f32),
}

/// Encoders: centre x, y, radius.
pub fn encs() -> [(f32, f32, f32); 8] {
    std::array::from_fn(|i| (862.0 + i as f32 * 86.0, 112.0, 31.0))
}

/// Pads: x, y, w, h (pad p: the top row 0..7).
pub fn pads() -> [(usize, usize, usize, usize); 16] {
    std::array::from_fn(|p| (824 + (p % 8) * 86, 186 + (p / 8) * 92, 76, 80))
}

/// Faders: the slot's x, top y, length (the cap travels it).
pub fn faders() -> [(usize, usize, usize); 4] {
    std::array::from_fn(|i| (1586 + i * 88, 74, 250))
}

/// The wheels: x, y, w, h (pitch, mod).
pub const WHEELS: [(usize, usize, usize, usize); 2] = [(56, 64, 76, 300), (158, 64, 76, 300)];

/// Buttons: (label, id, x, y, w, h, label below).
pub fn buttons() -> Vec<(&'static str, u8, usize, usize, usize, usize, bool)> {
    let b = |i: u8| BTN0 + i;
    let mut v = Vec::new();
    // left: KNOB BANK / FADER BANK, < / >, OCT- / OCT+
    for (r, pair) in [[(0, "KNOB BANK"), (1, "FADER BANK")], [(2, "<"), (3, ">")], [(4, "OCT-"), (5, "OCT+")]].iter().enumerate() {
        for (c, (i, name)) in pair.iter().enumerate() {
            v.push((*name, b(*i), 44 + c * 104, 418 + r * 58, 96, 44, false));
        }
    }
    // right, under the faders: ARP .. BT on top (labels above), PATCH .. SEQ REC below
    let top = [(6, "ARP"), (7, "NOTE REPEAT"), (8, "SCALE"), (9, "CHORD"), (10, "GLOBE"), (11, "BT")];
    let bot = [(12, "PATCH"), (13, "PARA"), (14, "FX"), (15, "SEQ"), (16, "SEQ PLAY"), (17, "SEQ REC")];
    for (c, (i, name)) in top.iter().enumerate() {
        v.push((*name, b(*i), 1560 + c * 56, 410, 48, 44, false));
    }
    for (c, (i, name)) in bot.iter().enumerate() {
        v.push((*name, b(*i), 1560 + c * 56, 466, 48, 44, true));
    }
    v
}

/// Keys: (id, x, y, w, h, black). 22 white keys C3..C6, the black ones over their gaps.
pub fn keys() -> Vec<(u8, usize, usize, usize, usize, bool)> {
    let (x0, y0, kw) = (40usize, 600usize, 83usize);
    let mut v = Vec::new();
    let mut white = 0usize;
    for k in 0..NKEY as u8 {
        let black = matches!(k % 12, 1 | 3 | 6 | 8 | 10);
        if black {
            let cx = x0 + white * kw;
            v.push((k, cx - 26, y0, 52, 168, true));
        } else {
            v.push((k, x0 + white * kw + 1, y0, kw - 2, 270, false));
            white += 1;
        }
    }
    v
}

pub fn hit(x: f32, y: f32) -> Option<Ctl> {
    for (i, (cx, cy, r)) in encs().iter().enumerate() {
        if (x - cx).powi(2) + (y - cy).powi(2) <= (r + 8.0).powi(2) {
            return Some(Ctl::Enc(i));
        }
    }
    let inside = |px: usize, py: usize, w: usize, h: usize| x >= px as f32 && x < (px + w) as f32 && y >= py as f32 && y < (py + h) as f32;
    for (p, (px, py, w, h)) in pads().iter().enumerate() {
        if inside(*px, *py, *w, *h) {
            return Some(Ctl::Pad(p));
        }
    }
    for (i, (fx, fy, len)) in faders().iter().enumerate() {
        if inside(fx - 30, fy - 20, 60, len + 40) {
            return Some(Ctl::Fader(i));
        }
    }
    if inside(WHEELS[0].0, WHEELS[0].1, WHEELS[0].2, WHEELS[0].3) {
        return Some(Ctl::Pitch);
    }
    if inside(WHEELS[1].0, WHEELS[1].1, WHEELS[1].2, WHEELS[1].3) {
        return Some(Ctl::Mod);
    }
    for (_, id, px, py, w, h, _) in buttons() {
        if inside(px, py, w, h) {
            return Some(Ctl::Btn(id));
        }
    }
    let mut ks = keys();
    ks.sort_by_key(|k| !k.5);
    for (id, px, py, w, h, _) in ks {
        if inside(px, py, w, h) {
            return Some(Ctl::Key(id, (y - py as f32) / h as f32));
        }
    }
    None
}

/// A fader's 0..1023 at panel y (the top is 1023).
pub fn fader_value(i: usize, y: f32) -> u16 {
    let (_, fy, len) = faders()[i];
    ((1.0 - (y - fy as f32) / len as f32).clamp(0.0, 1.0) * 1023.0).round() as u16
}

/// The wheels' raw reading at panel y: pitch 445..640 around its rest, mod 629 (down) .. 474 (up)
/// (fm1-to-smk37 docs/01 6.5).
pub fn wheel_value(pitch: bool, y: f32) -> u16 {
    let (_, wy, _, wh) = WHEELS[if pitch { 0 } else { 1 }];
    let t = ((y - wy as f32) / wh as f32).clamp(0.0, 1.0); // 0 top .. 1 bottom
    if pitch {
        (640.0 - t * (640.0 - 445.0)).round() as u16
    } else {
        (474.0 + t * (629.0 - 474.0)).round() as u16
    }
}

/// The parts that never change.
pub fn base() -> Vec<u32> {
    canvas(W);
    let mut b = vec![ROOM; W * H];
    rrect(&mut b, 14.0, 14.0, (W - 28) as f32, (H - 28) as f32, 30.0, 0x2c2f35, 0x202226);
    // the screen's bezel
    rrect(&mut b, (LCD_X - 18) as f32, (LCD_Y - 18) as f32, 516.0, 516.0, 20.0, 0x0e0e10, 0x0b0b0c);
    // the wheels' wells and the button trays
    rrect(&mut b, 44.0, 52.0, 202.0, 324.0, 16.0, TRAY, TRAY);
    rrect(&mut b, 34.0, 408.0, 222.0, 178.0, 14.0, TRAY, TRAY);
    rrect(&mut b, 1550.0, 384.0, 344.0, 160.0, 14.0, TRAY, TRAY);
    rrect(&mut b, 810.0, 172.0, 706.0, 202.0, 16.0, TRAY, TRAY);
    // the keybed's well
    rrect(&mut b, 28.0, 588.0, (W - 56) as f32, 296.0, 18.0, 0x16171a, 0x121315);
    text(&mut b, WHEELS[0].0 as f32 + 38.0, 384.0 - 2.0, "PITCH", 2, SILK);
    text(&mut b, WHEELS[1].0 as f32 + 38.0, 384.0 - 2.0, "MOD", 2, SILK);
    for (i, (cx, cy, r)) in encs().iter().enumerate() {
        text(&mut b, *cx, cy - r - 24.0, &format!("K{}", i + 1), 2, SILK);
    }
    for (i, (fx, fy, len)) in faders().iter().enumerate() {
        text(&mut b, *fx as f32, *fy as f32 - 40.0, &format!("F{}", i + 1), 2, SILK);
        rrect(&mut b, *fx as f32 - 5.0, *fy as f32, 10.0, *len as f32, 5.0, 0x0c0c0e, 0x0c0c0e);
    }
    for (name, _, x, y, w, h, below) in buttons() {
        let ly = if below { (y + h + 6) as f32 } else { y as f32 - 14.0 };
        if x > 1000 {
            text(&mut b, (x + w / 2) as f32, ly, name, 1, SILK);
        }
    }
    text(&mut b, 1160.0, 400.0, "SMK-37", 3, 0x7c7a75);
    b
}

pub struct State<'a> {
    pub lcd: &'a [u16],
    pub enc_clicks: &'a [i32; 8],
    pub turning: &'a [u32; 8],
    /// keys 0..36, buttons 37..54
    pub down: &'a [bool; 64],
    /// button LEDs 0..18 (LED k = button k), lit fraction
    pub leds: &'a [f32; 19],
    /// the pads' colours (0xRRGGBB) and how hard each is pressed (0..1023)
    pub pads: &'a [u32; 16],
    pub pad_force: &'a [u16; 16],
    pub faders: &'a [u16; 4],
    pub pitch: u16,
    pub modw: u16,
}

pub fn draw(buf: &mut [u32], base: &[u32], st: &State) {
    canvas(W);
    buf.copy_from_slice(base);
    for y in 0..240 {
        for x in 0..240 {
            let p = st.lcd[y * 240 + x] as u32;
            let c = ((p >> 11) * 255 / 31) << 16 | (((p >> 5) & 63) * 255 / 63) << 8 | (p & 31) * 255 / 31;
            let o = (LCD_Y + 2 * y) * W + LCD_X + 2 * x;
            buf[o] = c;
            buf[o + 1] = c;
            buf[o + W] = c;
            buf[o + W + 1] = c;
        }
    }
    for (i, (cx, cy, r)) in encs().iter().enumerate() {
        let glow = (st.turning[i] as f32 / 10.0).min(1.0);
        knob(buf, *cx, *cy, *r, (st.enc_clicks[i] as f32 * 18.0).to_radians(), glow);
    }
    // pads: their LED colour through a frosted top; pressed ones sink and brighten
    for (p, (x, y, w, h)) in pads().iter().enumerate() {
        let c = st.pads[p];
        let force = st.pad_force[p] as f32 / 1023.0;
        let off = if force > 0.0 { 2.0 } else { 0.0 };
        let lit = |s: u32| {
            let v = ((c >> s) & 255) as f32 / 255.0;
            (v.powf(0.6) * 255.0) as u32
        };
        let rgb = lit(16) << 16 | lit(8) << 8 | lit(0);
        rrect(buf, *x as f32 + 1.0, *y as f32 + 3.0, *w as f32, *h as f32, 10.0, 0x141416, 0x141416);
        let top = mix(mix(0x3d3f45, rgb, if c == 0 { 0.0 } else { 0.85 }), 0xffffff, force * 0.3);
        rrect(buf, *x as f32, *y as f32 + off, *w as f32, *h as f32, 10.0, top, mix(top, 0x000000, 0.25));
        text(buf, (x + w / 2) as f32, (y + h / 2) as f32 - 7.0 + off, &format!("{}", p + 1), 2, mix(0x9c9a94, 0x000000, if c == 0 { 0.0 } else { 0.6 }));
    }
    // faders: a cap on the slot
    for (i, (fx, fy, len)) in faders().iter().enumerate() {
        let t = st.faders[i] as f32 / 1023.0;
        let cy = *fy as f32 + (1.0 - t) * *len as f32;
        rrect(buf, *fx as f32 - 28.0, cy - 13.0 + 3.0, 56.0, 26.0, 5.0, 0x141416, 0x141416);
        rrect(buf, *fx as f32 - 28.0, cy - 13.0, 56.0, 26.0, 5.0, 0x5a5d63, 0x3c3e43);
        rrect(buf, *fx as f32 - 26.0, cy - 1.0, 52.0, 2.0, 1.0, 0xd8d4cc, 0xd8d4cc);
    }
    // wheels: a ridged roller, its mark at the reading
    for (i, (x, y, w, h)) in WHEELS.iter().enumerate() {
        let t = if i == 0 { (640.0 - st.pitch as f32) / (640.0 - 445.0) } else { (st.modw as f32 - 474.0) / (629.0 - 474.0) };
        rrect(buf, *x as f32, *y as f32, *w as f32, *h as f32, 12.0, 0x3a3c41, 0x26282c);
        let phase = t * 6.0;
        for k in 0..14 {
            let ry = *y as f32 + 10.0 + ((k as f32 + phase).rem_euclid(14.0)) * (*h as f32 - 20.0) / 14.0;
            rrect(buf, *x as f32 + 8.0, ry, *w as f32 - 16.0, 3.0, 1.5, 0x4a4d53, 0x4a4d53);
        }
        let my = *y as f32 + 12.0 + t.clamp(0.0, 1.0) * (*h as f32 - 24.0);
        rrect(buf, *x as f32 + 4.0, my - 3.0, *w as f32 - 8.0, 6.0, 3.0, AMBER, AMBER);
    }
    // buttons: pressed sinks, the LED tints the cap
    for (name, id, x, y, w, h, _) in buttons() {
        let down = st.down[id as usize];
        // (one latch a frame of 16 carries the other group's bits: a ~6% ghost, kept dark)
        let led = ((st.leds[(id - BTN0) as usize] - 0.15) / 0.35).clamp(0.0, 1.0);
        let off = if down { 2.0 } else { 0.0 };
        rrect(buf, x as f32 + 1.0, y as f32 + 3.0, w as f32, h as f32, 8.0, 0x1c1d20, 0x1c1d20);
        rrect(buf, x as f32, y as f32 + off, w as f32, h as f32, 8.0, mix(BTN_TOP, AMBER, led * 0.85), mix(BTN_BOT, AMBER, led * 0.7));
        if x < 1000 {
            let ink = if led > 0.5 { 0x2a1a03 } else { BTN_INK };
            text(buf, (x + w / 2) as f32, (y + h / 2) as f32 - 4.0 + off, name, 1, ink);
        }
    }
    // keys: white, then black on top; held ones darken a little
    for (id, x, y, w, h, black) in keys() {
        let down = st.down[id as usize];
        let off = if down { 3.0 } else { 0.0 };
        if black {
            rrect(buf, x as f32 + 2.0, y as f32 + 4.0, w as f32, h as f32, 5.0, 0x0a0a0b, 0x0a0a0b);
            let (top, bot) = if down { (0x3a3020, 0x2a2418) } else { (0x2a2b2e, 0x111214) };
            rrect(buf, x as f32, y as f32 + off, w as f32, h as f32, 5.0, top, bot);
        } else {
            let (top, bot) = if down { (0xe8dcc6, 0xd9c49c) } else { (0xf2f0ea, 0xd8d6cf) };
            rrect(buf, x as f32, y as f32 + off, w as f32, h as f32, 6.0, top, bot);
        }
        if id == 0 || id == 12 || id == 24 || id == 36 {
            text(buf, (x + w / 2) as f32, (y + h) as f32 - 24.0, &format!("C{}", 3 + id / 12), 2, 0x8a877f);
        }
    }
}
