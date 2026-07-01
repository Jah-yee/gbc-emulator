// src/frontend/overlay.rs
//
// Dep-free real-time debug overlay drawn on the SDL3 2D canvas (no OpenGL, no
// GUI crate). Shows: audio buffer depth, per-channel APU level meters, and a
// text panel (speed, CPU registers, PPU LY/LCDC) rendered with an embedded 3x5
// bitmap font. Coordinates are in window pixels (canvas has no logical size).

use crate::cpu::Cpu;
use sdl3::pixels::Color;
use sdl3::render::{Canvas, FRect, Texture};
use sdl3::video::Window;

// Layout (window pixels). Game fills the left GAME_W x GAME_H; the debug panel
// docks to its right. See mod.rs (SCALE=4) and the window-resize logic.
pub const GAME_W: i32 = 160 * 4;
pub const GAME_H: i32 = 144 * 4;
pub const PANEL_W: i32 = 360;

/// Build a float rect from integer window coords (SDL3's renderer is float-based).
fn r(x: i32, y: i32, w: i32, h: i32) -> FRect {
    FRect::new(x as f32, y as f32, w as f32, h as f32)
}

/// Draw the docked debug panel to the right of the game. `queued_bytes` is the
/// audio backlog; `pct` is the measured emulation speed.
/// The tile sheet texture is 16x24 tiles of 8x8 = 128x192 px.
pub const TILE_TEX_W: u32 = 128;
pub const TILE_TEX_H: u32 = 192;

pub fn draw(
    canvas: &mut Canvas<Window>,
    cpu: &Cpu,
    queued_bytes: i32,
    pct: u32,
    tile_tex: &mut Texture,
) {
    let px = GAME_W; // panel left edge
    canvas.set_draw_color(Color::RGB(16, 16, 20));
    let _ = canvas.fill_rect(r(px, 0, PANEL_W, GAME_H));

    let white = Color::RGB(235, 235, 235);
    let dim = Color::RGB(150, 150, 165);
    let x0 = px + 8;
    let s = 3i32;
    let lh = 6 * s;

    // --- CPU / PPU state ---
    let reg = &cpu.registers;
    let lines = [
        format!("SPD {}%", pct),
        format!("PC {:04X} SP {:04X}", cpu.pc, cpu.sp),
        format!("AF {:02X}{:02X} BC {:02X}{:02X}", reg.a, reg.f, reg.b, reg.c),
        format!("DE {:02X}{:02X} HL {:02X}{:02X}", reg.d, reg.e, reg.h, reg.l),
        format!(
            "LY {:02X} LCDC {:02X}",
            cpu.memory.read_byte(0xFF44),
            cpu.memory.read_byte(0xFF40)
        ),
    ];
    let mut y = 8i32;
    for line in &lines {
        draw_text(canvas, x0, y, s, line, white);
        y += lh;
    }

    // --- audio buffer depth ---
    y += 12;
    draw_text(canvas, x0, y, 2, "AUDIO BUF", dim);
    y += 6 * 2 + 4;
    let bar_full = PANEL_W - 16;
    let frac = (queued_bytes as f32 / 65_536.0).clamp(0.0, 1.0);
    canvas.set_draw_color(Color::RGB(40, 40, 40));
    let _ = canvas.fill_rect(r(x0, y, bar_full, 10));
    canvas.set_draw_color(Color::RGB(220, 200, 40));
    let _ = canvas.fill_rect(r(x0, y, (bar_full as f32 * frac) as i32, 10));

    // --- per-channel APU level meters ---
    y += 22;
    draw_text(canvas, x0, y, 2, "CH LEVELS", dim);
    y += 6 * 2 + 6;
    let levels = cpu.memory.apu.channel_levels();
    let bw = 30i32;
    let gap = 18i32;
    let mh = 56i32;
    for (i, &lvl) in levels.iter().enumerate() {
        let bx = x0 + i as i32 * (bw + gap);
        canvas.set_draw_color(Color::RGB(40, 40, 40));
        let _ = canvas.fill_rect(r(bx, y, bw, mh));

        let h = (lvl.clamp(0.0, 1.0) * mh as f32) as i32;
        let col = if cpu.memory.apu.is_muted(i) {
            Color::RGB(120, 40, 40)
        } else {
            Color::RGB(60, 220, 90)
        };
        canvas.set_draw_color(col);
        let _ = canvas.fill_rect(r(bx, y + (mh - h), bw, h));

        draw_text(canvas, bx + bw / 2 - 3, y + mh + 4, 2, &format!("{}", i + 1), white);
    }

    // --- VRAM tile viewer (384 tiles at 0x8000, bank 0) ---
    y += mh + 18;
    draw_text(canvas, x0, y, 2, "VRAM TILES", dim);
    y += 6 * 2 + 4;

    tile_tex
        .with_lock(None, |buf: &mut [u8], pitch: usize| {
            for t in 0..384usize {
                let tx = (t % 16) * 8; // 16 tiles per row
                let ty = (t / 16) * 8;
                let base = 0x8000 + (t * 16) as u16;
                for row in 0..8u16 {
                    let lo = cpu.memory.vram_read(0, base + row * 2);
                    let hi = cpu.memory.vram_read(0, base + row * 2 + 1);
                    for col in 0..8 {
                        let bit = 7 - col;
                        let id = (((hi >> bit) & 1) << 1) | ((lo >> bit) & 1);
                        let v = [255u8, 170, 85, 0][id as usize]; // 2bpp -> grayscale
                        let off = (ty + row as usize) * pitch + (tx + col) * 3;
                        buf[off] = v;
                        buf[off + 1] = v;
                        buf[off + 2] = v;
                    }
                }
            }
        })
        .unwrap();

    // Blit the sheet, keeping aspect and filling the remaining panel height.
    let avail = (GAME_H - y - 6).max(0) as f32;
    let sheet_w = avail * TILE_TEX_W as f32 / TILE_TEX_H as f32;
    let _ = canvas.copy(tile_tex, None, FRect::new(x0 as f32, y as f32, sheet_w, avail));
}

/// Render an uppercase/hex string with the embedded 3x5 font at `scale`x.
fn draw_text(canvas: &mut Canvas<Window>, x: i32, y: i32, scale: i32, text: &str, color: Color) {
    canvas.set_draw_color(color);
    let mut cx = x;
    for ch in text.chars() {
        let glyph = font3x5(ch);
        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..3i32 {
                if bits & (1 << (2 - col)) != 0 {
                    let _ = canvas.fill_rect(r(cx + col * scale, y + row as i32 * scale, scale, scale));
                }
            }
        }
        cx += 4 * scale; // 3 columns + 1 gap
    }
}

/// A minimal 3x5 bitmap font: uppercase, digits, and a few symbols. Each row's
/// low 3 bits are the columns (bit 2 = leftmost). Unknown chars render blank.
fn font3x5(c: char) -> [u8; 5] {
    match c {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b010],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b101, 0b111, 0b111, 0b111, 0b101],
        'O' => [0b010, 0b101, 0b101, 0b101, 0b010],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        'Q' => [0b010, 0b101, 0b101, 0b110, 0b011],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        ':' => [0b000, 0b010, 0b000, 0b010, 0b000],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        '%' => [0b101, 0b001, 0b010, 0b100, 0b101],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        '(' => [0b001, 0b010, 0b010, 0b010, 0b001],
        ')' => [0b100, 0b010, 0b010, 0b010, 0b100],
        _ => [0, 0, 0, 0, 0], // space + anything unmapped
    }
}
