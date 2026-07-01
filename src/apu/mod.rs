// src/apu/mod.rs
//
// Audio Processing Unit. All four channels:
//   Ch1 = square + frequency sweep    Ch2 = square
//   Ch3 = custom wave                 Ch4 = noise (LFSR)
// Registers live at 0xFF10-0xFF3F; Memory delegates those reads/writes here.

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

// Channel 4 noise divisors indexed by the low 3 bits of NR43.
const NOISE_DIVISORS: [i32; 8] = [8, 16, 32, 48, 64, 80, 96, 112];

/// A square-wave channel. Ch1 also uses the sweep fields; Ch2 leaves them zero.
#[derive(Default, Clone)]
struct Square {
    enabled: bool,

    freq: u16,
    freq_timer: i32,
    duty: u8,
    duty_pos: usize,

    length_counter: u8,
    length_enabled: bool,

    env_initial: u8,
    env_add: bool,
    env_period: u8,
    env_timer: u8,
    volume: u8,

    // Frequency sweep (Ch1 only).
    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift: u8,
    sweep_timer: u8,
    sweep_enabled: bool,
    sweep_shadow: u16,
}

impl Square {
    // NR10 (Ch1 only): sweep period (6-4), direction (3), shift (2-0).
    fn write_nr10(&mut self, v: u8) {
        self.sweep_period = (v >> 4) & 0x07;
        self.sweep_negate = v & 0x08 != 0;
        self.sweep_shift = v & 0x07;
    }

    fn write_nrx1(&mut self, v: u8) {
        self.duty = (v >> 6) & 0x03;
        self.length_counter = 64 - (v & 0x3F);
    }

    fn write_nrx2(&mut self, v: u8) {
        self.env_initial = v >> 4;
        self.env_add = v & 0x08 != 0;
        self.env_period = v & 0x07;
        if v & 0xF8 == 0 {
            self.enabled = false; // DAC off
        }
    }

    fn write_nrx3(&mut self, v: u8) {
        self.freq = (self.freq & 0x0700) | v as u16;
    }

    fn write_nrx4(&mut self, v: u8) {
        self.freq = (self.freq & 0x00FF) | (((v & 0x07) as u16) << 8);
        self.length_enabled = v & 0x40 != 0;
        if v & 0x80 != 0 {
            self.trigger();
        }
    }

    fn trigger(&mut self) {
        self.enabled = true;
        if self.length_counter == 0 {
            self.length_counter = 64;
        }
        self.freq_timer = (2048 - self.freq as i32) * 4;
        self.env_timer = self.env_period;
        self.volume = self.env_initial;

        // Sweep init (harmless for Ch2, which has zero sweep settings).
        self.sweep_shadow = self.freq;
        self.sweep_timer = if self.sweep_period > 0 { self.sweep_period } else { 8 };
        self.sweep_enabled = self.sweep_period > 0 || self.sweep_shift > 0;
        if self.sweep_shift > 0 {
            self.calc_sweep(); // immediate overflow check
        }
    }

    fn step(&mut self, cycles: i32) {
        self.freq_timer -= cycles;
        while self.freq_timer <= 0 {
            self.freq_timer += (2048 - self.freq as i32) * 4;
            self.duty_pos = (self.duty_pos + 1) & 7;
        }
    }

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

    // Compute the swept frequency; disables the channel on overflow past 2047.
    fn calc_sweep(&mut self) -> u16 {
        let delta = self.sweep_shadow >> self.sweep_shift;
        let new = if self.sweep_negate {
            self.sweep_shadow.wrapping_sub(delta)
        } else {
            self.sweep_shadow + delta
        };
        if new > 2047 {
            self.enabled = false;
        }
        new
    }

    fn clock_sweep(&mut self) {
        if !self.sweep_enabled {
            return;
        }
        if self.sweep_timer > 0 {
            self.sweep_timer -= 1;
        }
        if self.sweep_timer == 0 {
            self.sweep_timer = if self.sweep_period > 0 { self.sweep_period } else { 8 };
            if self.sweep_period > 0 {
                let new = self.calc_sweep();
                if new <= 2047 && self.sweep_shift > 0 {
                    self.sweep_shadow = new;
                    self.freq = new;
                    self.calc_sweep(); // second overflow check
                }
            }
        }
    }

    fn sample(&self) -> u8 {
        if !self.enabled {
            return 0;
        }
        DUTY[self.duty as usize][self.duty_pos] * self.volume
    }
}

/// Channel 3: plays 32 4-bit samples from wave RAM.
#[derive(Default, Clone)]
struct Wave {
    enabled: bool,
    dac_on: bool,
    freq: u16,
    freq_timer: i32,
    pos: usize,
    length_counter: u16,
    length_enabled: bool,
    volume_code: u8,
    ram: [u8; 16], // 32 nibbles
}

impl Wave {
    fn write_nr30(&mut self, v: u8) {
        self.dac_on = v & 0x80 != 0;
        if !self.dac_on {
            self.enabled = false;
        }
    }
    fn write_nr31(&mut self, v: u8) {
        self.length_counter = 256 - v as u16;
    }
    fn write_nr32(&mut self, v: u8) {
        self.volume_code = (v >> 5) & 0x03;
    }
    fn write_nr33(&mut self, v: u8) {
        self.freq = (self.freq & 0x0700) | v as u16;
    }
    fn write_nr34(&mut self, v: u8) {
        self.freq = (self.freq & 0x00FF) | (((v & 0x07) as u16) << 8);
        self.length_enabled = v & 0x40 != 0;
        if v & 0x80 != 0 {
            self.trigger();
        }
    }

    fn trigger(&mut self) {
        self.enabled = self.dac_on;
        if self.length_counter == 0 {
            self.length_counter = 256;
        }
        self.freq_timer = (2048 - self.freq as i32) * 2; // wave ticks twice as fast
        self.pos = 0;
    }

    fn step(&mut self, cycles: i32) {
        self.freq_timer -= cycles;
        while self.freq_timer <= 0 {
            self.freq_timer += (2048 - self.freq as i32) * 2;
            self.pos = (self.pos + 1) & 31;
        }
    }

    fn clock_length(&mut self) {
        if self.length_enabled && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    fn sample(&self) -> u8 {
        if !self.enabled || !self.dac_on {
            return 0;
        }
        let byte = self.ram[self.pos / 2];
        let nibble = if self.pos & 1 == 0 { byte >> 4 } else { byte & 0x0F };
        // Volume code: 0 = mute, 1 = 100%, 2 = 50%, 3 = 25% (right-shifts).
        let shift = [4, 0, 1, 2][self.volume_code as usize];
        nibble >> shift
    }
}

/// Channel 4: pseudo-random noise from a linear-feedback shift register.
#[derive(Default, Clone)]
struct Noise {
    enabled: bool,
    lfsr: u16,
    freq_timer: i32,
    width_7: bool,
    divisor_code: u8,
    clock_shift: u8,

    length_counter: u8,
    length_enabled: bool,

    env_initial: u8,
    env_add: bool,
    env_period: u8,
    env_timer: u8,
    volume: u8,
}

impl Noise {
    fn write_nr41(&mut self, v: u8) {
        self.length_counter = 64 - (v & 0x3F);
    }
    fn write_nr42(&mut self, v: u8) {
        self.env_initial = v >> 4;
        self.env_add = v & 0x08 != 0;
        self.env_period = v & 0x07;
        if v & 0xF8 == 0 {
            self.enabled = false;
        }
    }
    fn write_nr43(&mut self, v: u8) {
        self.clock_shift = v >> 4;
        self.width_7 = v & 0x08 != 0;
        self.divisor_code = v & 0x07;
    }
    fn write_nr44(&mut self, v: u8) {
        self.length_enabled = v & 0x40 != 0;
        if v & 0x80 != 0 {
            self.trigger();
        }
    }

    fn period(&self) -> i32 {
        NOISE_DIVISORS[self.divisor_code as usize] << self.clock_shift
    }

    fn trigger(&mut self) {
        self.enabled = true;
        if self.length_counter == 0 {
            self.length_counter = 64;
        }
        self.lfsr = 0x7FFF; // all bits set
        self.env_timer = self.env_period;
        self.volume = self.env_initial;
        self.freq_timer = self.period();
    }

    fn step(&mut self, cycles: i32) {
        self.freq_timer -= cycles;
        while self.freq_timer <= 0 {
            self.freq_timer += self.period();
            // XOR the low two bits, shift right, feed the result into bit 14
            // (and bit 6 in 7-bit "width" mode) for a shorter, buzzier period.
            let bit = (self.lfsr ^ (self.lfsr >> 1)) & 1;
            self.lfsr >>= 1;
            self.lfsr |= bit << 14;
            if self.width_7 {
                self.lfsr = (self.lfsr & !0x40) | (bit << 6);
            }
        }
    }

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

    fn sample(&self) -> u8 {
        if !self.enabled {
            return 0;
        }
        // Output is the inverted low bit of the LFSR, scaled by volume.
        ((!self.lfsr & 1) as u8) * self.volume
    }
}

#[derive(Clone)]
pub struct Apu {
    ch1: Square,
    ch2: Square,
    ch3: Wave,
    ch4: Noise,

    enabled: bool, // NR52 bit 7 master enable
    nr50: u8,      // master volume (+ VIN, unused): bits 6-4 left, 2-0 right
    nr51: u8,      // panning: bits 7-4 = channels to LEFT, 3-0 = channels to RIGHT

    fs_timer: u32,
    fs_step: u8,

    sample_clock: f32,
    hpf_l: f32, // high-pass filter capacitors (remove DC offset -> no pops/fuzz)
    hpf_r: f32,
    /// Generated interleaved stereo samples (L, R, L, R, ...), drained by the frontend.
    pub output: Vec<f32>,
}

/// One-pole high-pass filter: removes the DC offset so silence sits at 0 and
/// channel toggles don't click. `charge` ~ 0.999958^95 for a 44.1kHz sample.
fn high_pass(cap: &mut f32, sample: f32) -> f32 {
    let out = sample - *cap;
    *cap = sample - out * 0.996;
    out
}

impl Apu {
    pub fn new() -> Self {
        Apu {
            ch1: Square::default(),
            ch2: Square::default(),
            ch3: Wave::default(),
            ch4: Noise::default(),
            enabled: true,
            nr50: 0x77, // full volume both sides (post-boot default)
            nr51: 0xFF, // all channels to both sides
            fs_timer: 0,
            fs_step: 0,
            sample_clock: 0.0,
            hpf_l: 0.0,
            hpf_r: 0.0,
            output: Vec::new(),
        }
    }

    pub fn read_reg(&self, addr: u16) -> u8 {
        match addr {
            // NR52: master enable + per-channel on flags.
            0xFF26 => {
                let mut v = 0x70;
                if self.enabled {
                    v |= 0x80;
                }
                if self.ch1.enabled {
                    v |= 0x01;
                }
                if self.ch2.enabled {
                    v |= 0x02;
                }
                if self.ch3.enabled {
                    v |= 0x04;
                }
                if self.ch4.enabled {
                    v |= 0x08;
                }
                v
            }
            // Wave RAM.
            0xFF30..=0xFF3F => self.ch3.ram[(addr - 0xFF30) as usize],
            _ => 0xFF,
        }
    }

    pub fn write_reg(&mut self, addr: u16, value: u8) {
        match addr {
            // Channel 1 (square + sweep).
            0xFF10 => self.ch1.write_nr10(value),
            0xFF11 => self.ch1.write_nrx1(value),
            0xFF12 => self.ch1.write_nrx2(value),
            0xFF13 => self.ch1.write_nrx3(value),
            0xFF14 => self.ch1.write_nrx4(value),

            // Channel 2 (square).
            0xFF16 => self.ch2.write_nrx1(value),
            0xFF17 => self.ch2.write_nrx2(value),
            0xFF18 => self.ch2.write_nrx3(value),
            0xFF19 => self.ch2.write_nrx4(value),

            // Channel 3 (wave) + wave RAM.
            0xFF1A => self.ch3.write_nr30(value),
            0xFF1B => self.ch3.write_nr31(value),
            0xFF1C => self.ch3.write_nr32(value),
            0xFF1D => self.ch3.write_nr33(value),
            0xFF1E => self.ch3.write_nr34(value),
            0xFF30..=0xFF3F => self.ch3.ram[(addr - 0xFF30) as usize] = value,

            // Channel 4 (noise).
            0xFF20 => self.ch4.write_nr41(value),
            0xFF21 => self.ch4.write_nr42(value),
            0xFF22 => self.ch4.write_nr43(value),
            0xFF23 => self.ch4.write_nr44(value),

            0xFF24 => self.nr50 = value,
            0xFF25 => self.nr51 = value,
            0xFF26 => self.enabled = value & 0x80 != 0,
            _ => {}
        }
    }

    pub fn step(&mut self, cycles: u32) {
        let c = cycles as i32;
        self.ch1.step(c);
        self.ch2.step(c);
        self.ch3.step(c);
        self.ch4.step(c);

        // Frame sequencer: 512 Hz. Length on 0/2/4/6, sweep on 2/6, envelope on 7.
        self.fs_timer += cycles;
        while self.fs_timer >= FRAME_SEQ_PERIOD {
            self.fs_timer -= FRAME_SEQ_PERIOD;
            match self.fs_step {
                0 | 4 => {
                    self.ch1.clock_length();
                    self.ch2.clock_length();
                    self.ch3.clock_length();
                    self.ch4.clock_length();
                }
                2 | 6 => {
                    self.ch1.clock_length();
                    self.ch2.clock_length();
                    self.ch3.clock_length();
                    self.ch4.clock_length();
                    self.ch1.clock_sweep();
                }
                7 => {
                    self.ch1.clock_envelope();
                    self.ch2.clock_envelope();
                    self.ch4.clock_envelope();
                }
                _ => {}
            }
            self.fs_step = (self.fs_step + 1) & 7;
        }

        self.sample_clock += cycles as f32;
        while self.sample_clock >= CYCLES_PER_SAMPLE {
            self.sample_clock -= CYCLES_PER_SAMPLE;
            let (l, r) = self.mix_stereo();
            let l = high_pass(&mut self.hpf_l, l);
            let r = high_pass(&mut self.hpf_r, r);
            self.output.push(l);
            self.output.push(r);
        }
    }

    /// Mix the four channels into a stereo pair, honoring NR51 panning and NR50
    /// master volume. Each side is roughly [0.0, 1.0] before the high-pass filter
    /// re-centers it around 0.
    fn mix_stereo(&self) -> (f32, f32) {
        if !self.enabled {
            return (0.0, 0.0);
        }
        let samples = [
            self.ch1.sample(),
            self.ch2.sample(),
            self.ch3.sample(),
            self.ch4.sample(),
        ];
        let mut left = 0.0f32;
        let mut right = 0.0f32;
        for (i, &s) in samples.iter().enumerate() {
            let a = s as f32 / 15.0; // 0.0..1.0
            if self.nr51 & (1 << (i + 4)) != 0 {
                left += a; // NR51 bits 7-4 route channels to the left
            }
            if self.nr51 & (1 << i) != 0 {
                right += a; // NR51 bits 3-0 route channels to the right
            }
        }
        // Master volume 0-7 per side -> scale (vol+1)/8; /4 normalizes 4 channels.
        let lvol = (((self.nr50 >> 4) & 0x07) as f32 + 1.0) / 8.0;
        let rvol = ((self.nr50 & 0x07) as f32 + 1.0) / 8.0;
        (left / 4.0 * lvol, right / 4.0 * rvol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_square_produces_a_waveform() {
        let mut ch = Square::default();
        ch.write_nrx1(0x80);
        ch.write_nrx2(0xF0);
        ch.write_nrx3(0x00);
        ch.write_nrx4(0x87);
        assert!(ch.enabled);
        let mut saw_high = false;
        let mut saw_low = false;
        for _ in 0..64 {
            ch.step(64);
            if ch.sample() == 0 {
                saw_low = true;
            } else {
                saw_high = true;
            }
        }
        assert!(saw_high && saw_low);
    }

    #[test]
    fn test_length_counter_disables_channel() {
        let mut ch = Square::default();
        ch.write_nrx2(0xF0);
        ch.write_nrx1(0x3F);
        ch.write_nrx4(0xC7);
        assert!(ch.enabled);
        ch.clock_length();
        assert!(!ch.enabled);
    }

    #[test]
    fn test_sweep_disables_on_overflow() {
        let mut ch = Square::default();
        ch.write_nrx2(0xF0); // DAC on
        ch.freq = 2000;
        ch.write_nr10(0x11); // period 1, increase, shift 1
        ch.write_nrx4(0x87); // trigger
        // 2000 + (2000 >> 1) = 3000 > 2047 -> overflow disables the channel.
        ch.clock_sweep();
        assert!(!ch.enabled);
    }

    #[test]
    fn test_wave_plays_ram_samples() {
        let mut ch = Wave::default();
        ch.write_nr30(0x80); // DAC on
        ch.ram[0] = 0xF0; // first two nibbles: 0xF, 0x0
        ch.write_nr32(0x20); // volume 100%
        ch.write_nr34(0x80); // trigger
        assert_eq!(ch.sample(), 0xF); // pos 0 -> high nibble
        ch.step((2048 - ch.freq as i32) * 2); // advance one sample
        assert_eq!(ch.sample(), 0x0); // pos 1 -> low nibble
    }

    #[test]
    fn test_noise_generates_bits() {
        let mut ch = Noise::default();
        ch.write_nr42(0xF0); // volume 15
        ch.write_nr43(0x00); // fastest-ish
        ch.write_nr44(0x80); // trigger
        assert!(ch.enabled);
        let mut changed = false;
        let first = ch.sample();
        for _ in 0..64 {
            ch.step(64);
            if ch.sample() != first {
                changed = true;
            }
        }
        assert!(changed, "noise output should vary");
    }

    #[test]
    fn test_apu_generates_samples() {
        let mut apu = Apu::new();
        apu.write_reg(0xFF17, 0xF0);
        apu.write_reg(0xFF19, 0x87);
        apu.step(10_000);
        // ~10000/95 ≈ 105 sample points, x2 for interleaved stereo.
        assert!(apu.output.len() > 180 && apu.output.len() < 240);
    }
}
