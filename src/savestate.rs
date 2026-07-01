// src/savestate.rs
//
// Whole-machine save states: bincode-encode the entire Cpu (which owns Memory
// and Apu) with a small header so stale/incompatible files are rejected cleanly.
// bincode's native derive handles our large fixed arrays (serde only does <=32).

use crate::cpu::Cpu;

const MAGIC: u32 = 0x4742_4353; // "GBCS"
const VERSION: u32 = 1;

/// Serialize the whole machine: magic + version + bincode payload.
pub fn to_bytes(cpu: &Cpu) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.extend_from_slice(&VERSION.to_le_bytes());
    let payload = bincode::encode_to_vec(cpu, bincode::config::standard())
        .expect("save-state encode should not fail");
    out.extend_from_slice(&payload);
    out
}

/// Rebuild a machine from save-state bytes. Returns None on a bad/incompatible
/// header or a decode failure.
pub fn from_bytes(bytes: &[u8]) -> Option<Cpu> {
    if bytes.len() < 8 {
        return None;
    }
    let magic = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
    let version = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    if magic != MAGIC || version != VERSION {
        return None;
    }
    let (cpu, _) = bincode::decode_from_slice(&bytes[8..], bincode::config::standard()).ok()?;
    Some(cpu)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_state() {
        // Decoding builds a whole Cpu (~150 KB) on the stack; the test-harness
        // thread only gets 2 MB, so run on a roomier stack. The real app loads on
        // the main thread (8 MB), so this is a test-only concern.
        std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(|| {
                let mut cpu = Cpu::new();
                cpu.registers.a = 0x42;
                cpu.cycles = 123_456;
                cpu.framebuffer[0] = (10, 20, 30);

                let bytes = to_bytes(&cpu);
                let restored = from_bytes(&bytes).expect("decode should succeed");

                assert_eq!(restored.registers.a, 0x42);
                assert_eq!(restored.cycles, 123_456);
                assert_eq!(restored.framebuffer[0], (10, 20, 30));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn rejects_bad_header() {
        assert!(from_bytes(&[0, 1, 2, 3, 4, 5, 6, 7]).is_none());
        assert!(from_bytes(&[]).is_none());
    }
}
