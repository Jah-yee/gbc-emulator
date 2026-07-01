// src/apu/mod.rs
//
// Audio Processing Unit. Chunk 1: the output pipeline + Channel 2 (a square
// wave with duty cycle, length counter, and volume envelope). Registers live at
// 0xFF10-0xFF3F; Memory delegates those reads/writes here.

const CPU_HZ: f32 = 4_194_304.0;
const SAMPLE_RATE: f32 = 44_100.0;
const CYCLES_PER_SAMPLE: f32 = CPU_HZ / SAMPLE_RATE;
const FRAME_SEQ_PERIOD: u32 = 8192; // 512 Hz

// Duty patterns: 8 steps each, 1 = high. (12.5%, 25%, 50%, 75%.)
const DUTY: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 0],
];

/// A square-wave channel (used by Ch2; Ch1 will reuse it plus sweep later).
#[derive(Default)]
struct Square {
    enabled: bool,

    // Frequency: 11-bit value; timer reloads with (2048 - freq) * 4 CPU cycles.
    freq: u16,
    freq_timer: i32,
    duty: u8,
    duty_pos: usize,

    // Length counter: when enabled, counts down and disables the channel at 0.
    length_counter: u8,
    length_enabled: bool,

    // Volume envelope.
    env_initial: u8,   // starting volume 0-15
    env_add: bool,     // true = increase, false = decrease
    env_period: u8,    // ticks between volume steps (0 = off)
    env_timer: u8,     // countdown to the next step
    volume: u8,        // current volume 0-15
}

impl Square {
    // NRx1: bits 7-6 duty, bits 5-0 length load.
    fn write_nrx1(&mut self, v: u8) {
        self.duty = (v >> 6) & 0x03;
        self.length_counter = 64 - (v & 0x3F); // length counts up to 64
    }

    // NRx2: bits 7-4 initial volume, bit 3 direction, bits 2-0 period.
    fn write_nrx2(&mut self, v: u8) {
        self.env_initial = v >> 4;
        self.env_add = v & 0x08 != 0;
        self.env_period = v & 0x07;
        // Writing 0 to the top 5 bits turns the channel's DAC off.
        if v & 0xF8 == 0 {
            self.enabled = false;
        }
    }

    // NRx3: low 8 bits of frequency.
    fn write_nrx3(&mut self, v: u8) {
        self.freq = (self.freq & 0x0700) | v as u16;
    }

    // NRx4: bit 7 trigger, bit 6 length enable, bits 2-0 freq high bits.
    fn write_nrx4(&mut self, v: u8) {
        self.freq = (self.freq & 0x00FF) | (((v & 0x07) as u16) << 8);
        self.length_enabled = v & 0x40 != 0;
        if v & 0x80 != 0 {
            self.trigger();
        }
    }

    // A "trigger" (re)starts the channel: enable, reload timers, reset envelope.
    fn trigger(&mut self) {
        self.enabled = true;
        if self.length_counter == 0 {
            self.length_counter = 64;
        }
        self.freq_timer = (2048 - self.freq as i32) * 4;
        self.env_timer = self.env_period;
        self.volume = self.env_initial;
    }

    // Advance the frequency timer by `cycles`, stepping the duty position.
    fn step(&mut self, cycles: i32) {
        self.freq_timer -= cycles;
        while self.freq_timer <= 0 {
            self.freq_timer += (2048 - self.freq as i32) * 4;
            self.duty_pos = (self.duty_pos + 1) & 7;
        }
    }

    // Frame sequencer hooks.
    fn clock_length(&mut self) {
        if self.length_enabled && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    fn clock_envelope(&mut self) {
        if self.env_period == 0 {
            return;
        }
        if self.env_timer > 0 {
            self.env_timer -= 1;
        }
        if self.env_timer == 0 {
            self.env_timer = self.env_period;
            if self.env_add && self.volume < 15 {
                self.volume += 1;
            } else if !self.env_add && self.volume > 0 {
                self.volume -= 1;
            }
        }
    }

    // Current output amplitude, 0..15 (0 when off).
    fn sample(&self) -> u8 {
        if !self.enabled {
            return 0;
        }
        DUTY[self.duty as usize][self.duty_pos] * self.volume
    }
}

pub struct Apu {
    ch2: Square,

    // Master enable (NR52 bit 7).
    enabled: bool,

    // Frame sequencer.
    fs_timer: u32,
    fs_step: u8,

    // Output sampling.
    sample_clock: f32,
    /// Generated mono samples in [-1.0, 1.0], drained by the frontend.
    pub output: Vec<f32>,
}

impl Apu {
    pub fn new() -> Self {
        Apu {
            ch2: Square::default(),
            enabled: true,
            fs_timer: 0,
            fs_step: 0,
            sample_clock: 0.0,
            output: Vec::new(),
        }
    }

    pub fn read_reg(&self, addr: u16) -> u8 {
        match addr {
            // NR52: bit 7 = master enable, bits 0-3 = channel-on status.
            0xFF26 => {
                let mut v = 0x70; // unused bits read 1
                if self.enabled {
                    v |= 0x80;
                }
                if self.ch2.enabled {
                    v |= 0x02;
                }
                v
            }
            _ => 0xFF, // other regs: readback not modeled yet
        }
    }

    pub fn write_reg(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF16 => self.ch2.write_nrx1(value),
            0xFF17 => self.ch2.write_nrx2(value),
            0xFF18 => self.ch2.write_nrx3(value),
            0xFF19 => self.ch2.write_nrx4(value),
            0xFF26 => self.enabled = value & 0x80 != 0,
            _ => {}
        }
    }

    /// Advance the APU by the cycles the last instruction took, generating
    /// output samples as we cross sample boundaries.
    pub fn step(&mut self, cycles: u32) {
        self.ch2.step(cycles as i32);

        // Frame sequencer: 512 Hz, stepping length/envelope/sweep.
        self.fs_timer += cycles;
        while self.fs_timer >= FRAME_SEQ_PERIOD {
            self.fs_timer -= FRAME_SEQ_PERIOD;
            match self.fs_step {
                0 | 4 => self.ch2.clock_length(),
                2 | 6 => self.ch2.clock_length(), // (sweep also lands here later)
                7 => self.ch2.clock_envelope(),
                _ => {}
            }
            self.fs_step = (self.fs_step + 1) & 7;
        }

        // Emit output samples at ~44.1kHz.
        self.sample_clock += cycles as f32;
        while self.sample_clock >= CYCLES_PER_SAMPLE {
            self.sample_clock -= CYCLES_PER_SAMPLE;
            self.output.push(self.mix());
        }
    }

    /// Mix enabled channels into a single sample. Silence is 0.0 (not -1.0, which
    /// would be a DC offset that clicks/pops). Scaled down to leave headroom for
    /// the other three channels arriving in chunk 2.
    fn mix(&self) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        (self.ch2.sample() as f32 / 15.0) * 0.25 // amplitude 0..15 -> 0.0..0.25
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_square_produces_a_waveform() {
        let mut ch = Square::default();
        ch.write_nrx1(0x80); // duty 2 (50%), length load 0
        ch.write_nrx2(0xF0); // volume 15, no envelope step
        ch.write_nrx3(0x00);
        ch.write_nrx4(0x87); // trigger, freq high = 7 -> freq 0x700

        assert!(ch.enabled);
        // Step through a full period and confirm the output isn't constant
        // (a real square wave alternates between 0 and the volume).
        let mut saw_high = false;
        let mut saw_low = false;
        for _ in 0..64 {
            ch.step(64);
            match ch.sample() {
                0 => saw_low = true,
                _ => saw_high = true,
            }
        }
        assert!(saw_high && saw_low, "square wave should alternate high/low");
    }

    #[test]
    fn test_length_counter_disables_channel() {
        let mut ch = Square::default();
        ch.write_nrx2(0xF0); // DAC on
        ch.write_nrx1(0x3F); // length load 63 -> counter = 1
        ch.write_nrx4(0xC7); // trigger + length enable
        assert!(ch.enabled);
        ch.clock_length(); // counter 1 -> 0
        assert!(!ch.enabled, "length counter reaching 0 disables the channel");
    }

    #[test]
    fn test_apu_generates_samples() {
        let mut apu = Apu::new();
        apu.write_reg(0xFF17, 0xF0); // ch2 volume
        apu.write_reg(0xFF19, 0x87); // trigger
        apu.step(10_000);
        // ~10000 / 95 ≈ 105 samples.
        assert!(apu.output.len() > 90 && apu.output.len() < 120);
    }
}
