//! Harness for Blargg's CPU test ROMs.
//!
//! Drop the individual `cpu_instrs` ROMs (e.g. `01-special.gb`) into a
//! `test-roms/` directory at the crate root, then `cargo test`. Each ROM runs
//! until it reports a result over the serial port (captured in `Memory.serial`)
//! or until a cycle cap acts as a timeout. Missing ROMs are skipped, not failed,
//! so the suite stays green for anyone who doesn't have them.

use crate::cpu::Cpu;

/// Generous upper bound on emulated cycles before we call it a hang. On success
/// the ROM reports far sooner and we stop early; this only bites on a broken CPU.
const MAX_CYCLES: u64 = 250_000_000;

/// Run a ROM until its serial output reports Passed/Failed, or the cap is hit.
/// Returns `None` if the ROM file isn't present (so the caller can skip).
fn run_test_rom(path: &str) -> Option<String> {
    let rom = std::fs::read(path).ok()?;
    let mut cpu = Cpu::new();
    cpu.memory.load_rom(&rom);

    let mut last_len = 0;
    while cpu.cycles < MAX_CYCLES {
        cpu.step();
        // Only rescan the buffer when new serial bytes have arrived.
        let len = cpu.memory.serial.len();
        if len != last_len {
            last_len = len;
            if cpu.memory.serial.contains("Passed") || cpu.memory.serial.contains("Failed") {
                break;
            }
        }
    }
    Some(cpu.memory.serial.clone())
}

/// Assert a Blargg ROM passes, or skip if it isn't installed.
fn check(rom: &str) {
    let path = format!("test-roms/{rom}");
    match run_test_rom(&path) {
        None => eprintln!("SKIP {rom}: not found at {path} (drop Blargg's cpu_instrs ROMs there)"),
        Some(output) => assert!(output.contains("Passed"), "{rom} did not pass:\n{output}"),
    }
}

#[test]
fn blargg_01_special() {
    check("01-special.gb");
}

#[test]
fn blargg_02_interrupts() {
    check("02-interrupts.gb");
}

#[test]
fn blargg_03_op_sp_hl() {
    check("03-op sp,hl.gb");
}

#[test]
fn blargg_04_op_r_imm() {
    check("04-op r,imm.gb");
}

#[test]
fn blargg_05_op_rp() {
    check("05-op rp.gb");
}

#[test]
fn blargg_06_ld_r_r() {
    check("06-ld r,r.gb");
}

#[test]
fn blargg_07_jr_jp_call_ret_rst() {
    check("07-jr,jp,call,ret,rst.gb");
}

#[test]
fn blargg_08_misc_instrs() {
    check("08-misc instrs.gb");
}

#[test]
fn blargg_09_op_r_r() {
    check("09-op r,r.gb");
}

#[test]
fn blargg_10_bit_ops() {
    check("10-bit ops.gb");
}

#[test]
fn blargg_11_op_a_hl() {
    check("11-op a,(hl).gb");
}

#[test]
fn blargg_instr_timing() {
    check("instr_timing.gb");
}
