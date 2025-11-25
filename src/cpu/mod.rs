// src/cpu/mod.rs

use crate::memory::Memory;

// The CPU registers
pub struct Registers {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8
}

impl Registers {
    pub fn new() -> Self {
        Self {
        a: 0,
        f: 0,
        b: 0,
        c: 0,
        d: 0,
        e: 0,
        h: 0,
        l: 0
        }   
    }
    // Helper methods to work with paired registers
    pub fn bc(&self) -> u16 {
        ((self.b as u16) << 8 ) | (self.c as u16)
    }

    pub fn set_bc(&mut self, value: u16) {
        self.b = (value >> 8) as u8;
        self.c = value as u8;
    }

    pub fn de(&self) -> u16 {
        ((self.d as u16) << 8) | (self.e as u16)
    }

    pub fn set_de(&mut self, value: u16) {
        self.d = (value >> 8) as u8;
        self.e = value as u8;
    }

    pub fn hl(&self) -> u16 {
        ((self.h as u16) <<8) | (self.l as u16)
    }

    pub fn set_hl(&mut self, value: u16) {
        self.h = (value >>8) as u8;
        self.l = value as u8;
    }

    // Flag manipulation
    pub fn flag_zero(&self) -> bool {
        self.f & 0b1000_0000 != 0
    }

    pub fn set_flag_zero(&mut self, set: bool) {
        if set {
            self.f |= 0b1000_0000;
        } else {
            self.f &= 0b0111_1111;
        }
    }

    pub fn flag_subtract(&self) -> bool {
        self.f & 0b0100_0000 != 0
    }

    pub fn set_flag_subtract(&mut self, set: bool) {
        if set {
            self.f |= 0b0100_0000;
        } else {
            self.f &= 0b1101_1111;
        }
    }

    pub fn flag_half_carry(&self) -> bool {
        self.f & 0b0010_0000 != 0
    }

    pub fn set_flag_half_carry(&mut self, set: bool) {
        if set {
            self.f |= 0b0010_0000;
        } else {
            self.f &= 0b1101_1111;
        }
    }

    pub fn flag_carry(&self) -> bool {
        self.f & 0b0001_0000 != 0
    }

    pub fn set_flag_carry(&mut self, set:bool) {
        if set {
            self.f |= 0b0001_0000;
        } else {
            self.f &= 0b1110_1111
        }
    }
}

pub struct Cpu {
    pub registers: Registers,
    pub pc: u16, // Program Counter
    pub sp: u16, // Stack Pointer
    pub memory: Memory,
    pub cycles: u64, // Total cycles executed
    pub halted: bool,
    pub interrupts_enabled: bool,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            registers: Registers::new(),
            pc: 0x0100, // Game start at 0x100 (after boot ROM)
            sp: 0xFFFE, // Stack starts at top of memory
            memory: Memory::new(),
            cycles: 0,
            halted: false,
            interrupts_enabled: false,
        }
    }

    /// Execute one instruction
    pub fn step(&mut self) {
        if self.halted {
            // When halted, just increment cycles
            self.cycles += 4;
            return;
        }

        // Fetch the opcode
        let opcode = self.fetch_byte();

        // Execute the instruction
        self.execute(opcode);
    }

    /// Fetch a byte from PC and increment PC
    fn fetch_byte(&mut self) -> u8 {
        let byte = self.memory.read_byte(self.pc);
        self.pc = self.pc.wrapping_add(1);
        byte
    }

    /// Fetch a 16-bit word from PC and increment PC twice
    fn fetch_word(&mut self) -> u16 {
        let word = self.memory.read_word(self.pc);
        self.pc = self.pc.wrapping_add(2);
        word
    }

    /// Execute an instruction based on opcode
    fn execute(&mut self, opcode: u8) {
        match opcode {
            // NOP - No Operation
            0x00 => {
                self.cycles += 4;
            }

            // LD BC, d16 - Load 16-bit immediate into BC
            0x01 => {
                let value = self.fetch_word();
                self.registers.set_bc(value);
                self.cycles += 12;
            }

            // LD (BC), A - Load A into memory address pointed to by BC
            0x02 => {
                let address = self.registers.bc();
                self.memory.write_byte(address, self.registers.a);
                self.cycles += 8;
            }

            // INC BC - Increment BC
            0x03 => {
                let value = self.registers.bc().wrapping_add(1);
                self.registers.set_bc(value);
                self.cycles += 8;
            }

            // INC B - Increment B
            0x04 => {
                self.registers.b = self.alu_inc(self.registers.b);
                self.cycles += 4;
            }

            // DEC B - Decrement B
            0x05 => {
                self.registers.b = self.alu_dec(self.registers.b);
                self.cycles += 4;
            }

            // LD B, d8 - Load 8-bit immediate into B
            0x06 => {
                self.registers.b = self.fetch_byte();
                self.cycles += 8;
            }

            0x76 => {
                self.halted = true;
                self.cycles += 4;
            }

            0x80 => {
                self.alu_add(self.registers.b);
                self.cycles += 4;
            }
            //...so I need to implement all 256 opcodes?

            0x3E => {
                self.registers.a = self.fetch_byte();
                self.cycles += 8;
            }

            _ => panic!("Unimplemented opcode: 0x{:02X} at PC: 0x{:04X}", opcode, self.pc -1),
        }
    }

    // ALU (Arithmetic Logic Unit) operations

    fn alu_inc(&mut self, value: u8) -> u8 {
        let result = value.wrapping_add(1);
        
        // Set flags
        self.registers.set_flag_zero(result == 0);
        self.registers.set_flag_subtract(false);
        self.registers.set_flag_half_carry((value & 0x0F) == 0x0F);
        
        result
    }

    fn alu_dec(&mut self, value: u8) -> u8 {
        let result = value.wrapping_sub(1);
        
        // Set flags
        self.registers.set_flag_zero(result == 0);
        self.registers.set_flag_subtract(true);
        self.registers.set_flag_half_carry((value & 0x0F) == 0);
        
        result
    }

    fn alu_add(&mut self, value: u8) {
        let a = self.registers.a;
        let result = a.wrapping_add(value);

        // Set flags
        self.registers.set_flag_zero(result == 0);
        self.registers.set_flag_subtract(false);
        self.registers.set_flag_half_carry((a & 0x0F) + (value & 0x0F) > 0x0F);
        self.registers.set_flag_carry(a as u16 + value as u16 > 0xFF);
        
        self.registers.a = result;
    }

    fn alu_sub(&mut self, value: u8) {
        let a = self.registers.a;
        let result = a.wrapping_sub(value);

        // Set flags
        self.registers.set_flag_zero(result == 0);
        self.registers.set_flag_subtract(true);
        self.registers.set_flag_half_carry((a & 0x0F) < (value & 0x0F));
        self.registers.set_flag_carry(a < value);
        
        self.registers.a = result;
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_pairing() {
        let mut registers = Registers::new();

        // Test BC pairing
        registers.b = 0x12;
        registers.c = 0x34;
        assert_eq!(registers.bc(), 0x1234);


        registers.set_bc(0xABCD);
        assert_eq!(registers.b, 0xAB);
        assert_eq!(registers.c, 0xCD);

        // Test DE pairing
        registers.d = 0x56;
        registers.e = 0x78;
        assert_eq!(registers.de(), 0x5678);


        registers.set_de(0xEF01);
        assert_eq!(registers.d, 0xEF);
        assert_eq!(registers.e, 0x01);

        // Test HL pairing
        registers.h = 0x9A;
        registers.l = 0xBC;
        assert_eq!(registers.hl(), 0x9ABC);


        registers.set_hl(0x2345);
        assert_eq!(registers.h, 0x23);
        assert_eq!(registers.l, 0x45);
    }

    #[test]
    fn test_flag_operations() {
        let mut registers = Registers::new();

        // All flags should start clear
        assert_eq!(registers.f, 0);
        assert!(!registers.flag_zero());
        assert!(!registers.flag_subtract());
        assert!(!registers.flag_half_carry());
        assert!(!registers.flag_carry());

        // Test zero flag
        registers.set_flag_zero(true);
        assert!(registers.flag_zero());
        assert_eq!(registers.f & 0b1000_0000, 0b1000_0000);

        registers.set_flag_zero(false);
        assert!(!registers.flag_zero());

        // Test subtract flag
        registers.set_flag_subtract(true);
        assert!(registers.flag_subtract());
        assert_eq!(registers.f & 0b0100_0000, 0b0100_0000);

        // Test half carry flag
        registers.set_flag_half_carry(true);
        assert!(registers.flag_half_carry());
        assert_eq!(registers.f & 0b0010_0000, 0b0010_0000);
        
        // Test carry flag
        registers.set_flag_carry(true);
        assert!(registers.flag_carry());
        assert_eq!(registers.f & 0b0001_0000, 0b0001_0000);

        // Test multiple flags at once
        registers.f = 0;
        registers.set_flag_zero(true);
        registers.set_flag_carry(true);
        assert!(registers.flag_zero());
        assert!(registers.flag_carry());
        assert!(!registers.flag_subtract());
        assert_eq!(registers.f, 0b1001_0000);
    }

    #[test]
    fn test_flag_lower_bits_always_zero() {
        let mut registers = Registers::new();

        // Set all flags
        registers.set_flag_zero(true);
        registers.set_flag_subtract(true);
        registers.set_flag_half_carry(true);
        registers.set_flag_carry(true);

        // Lower 4 bits should still be zero
        assert_eq!(registers.f & 0x0F, 0);
    }
}