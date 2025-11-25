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
        ((self.h as u16) <<8) | (self.c as u16)
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