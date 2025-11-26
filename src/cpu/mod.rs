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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Register {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
}

impl Register {
    /// Get the value of this register from a CPU
    pub fn get(&self, cpu: &Cpu) -> u8 {
        match self {
            Register::A => cpu.registers.a,
            Register::B => cpu.registers.b,
            Register::C => cpu.registers.c,
            Register::D => cpu.registers.d,
            Register::E => cpu.registers.e,
            Register::H => cpu.registers.h,
            Register::L => cpu.registers.l,
        }
    }

    /// Set the value of this register in a CPU
    pub fn set(&self, cpu: &mut Cpu, value: u8) {
        match self {
            Register::A => cpu.registers.a = value,
            Register::B => cpu.registers.b = value,
            Register::C => cpu.registers.c = value,
            Register::D => cpu.registers.d = value,
            Register::E => cpu.registers.e = value,
            Register::H => cpu.registers.h = value,
            Register::L => cpu.registers.l = value,
        }
    }

    /// Get the name of this register as a string
    pub fn name(&self) -> &'static str {
        match self {
            Register::A => "A",
            Register::B => "B",
            Register::C => "C",
            Register::D => "D",
            Register::E => "E",
            Register::H => "H",
            Register::L => "L",
        }
    }

    /// Get all registers as an array (useful for iteration)
    pub fn all() -> [Register; 7] {
        [
            Register::B,
            Register::C,
            Register::D,
            Register::E,
            Register::H,
            Register::L,
            Register::A,
        ]
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

            // INC C - Increment C
            0x0C => {
                self.registers.c = self.alu_inc(self.registers.c);
                self.cycles += 4;
            }

            // DEC C - Decrement C
            0x0D => {
                self.registers.c = self.alu_dec(self.registers.c);
                self.cycles += 4;
            }

            // LD C, d8
            0x0E => {  
                self.registers.c = self.fetch_byte();
                self.cycles += 8;
            }

            // INC D - Increment D
            0x14 => {
                self.registers.d = self.alu_inc(self.registers.d);
                self.cycles += 4;
            }

            // DEC D - Decrement D
            0x15 => {
                self.registers.d = self.alu_dec(self.registers.d);
                self.cycles += 4;
            }

            // LD D, d8
            0x16 => { 
                self.registers.d = self.fetch_byte();
                self.cycles += 8;
            }

            // INC E - Increment E
            0x1C => {
                self.registers.e = self.alu_inc(self.registers.e);
                self.cycles += 4;
            }

            // DEC E - Decrement E
            0x1D => {
                self.registers.e = self.alu_dec(self.registers.e);
                self.cycles += 4;
            }

            // LD E, d8
            0x1E => { 
                self.registers.e = self.fetch_byte();
                self.cycles += 8;
            }

            // INC H - Increment H
            0x24 => {
                self.registers.h = self.alu_inc(self.registers.h);
                self.cycles += 4;
            }

            // DEC H - Decrement H
            0x25 => {
                self.registers.h = self.alu_dec(self.registers.h);
                self.cycles += 4;
            }

            // LD H, d8
            0x26 => { 
                self.registers.h = self.fetch_byte();
                self.cycles += 8;
            }
            
            // INC L - Increment L
            0x2C => {
                self.registers.l = self.alu_inc(self.registers.l);
                self.cycles += 4;
            }

            // DEC L - Decrement L
            0x2D => {
                self.registers.l = self.alu_dec(self.registers.l);
                self.cycles += 4;
            }

            // LD L, d8
            0x2E => { 
                self.registers.l = self.fetch_byte();
                self.cycles += 8;
            }

            // INC A - Increment A
            0x3C => {
                self.registers.a = self.alu_inc(self.registers.a);
                self.cycles += 4;
            }
            
            // DEC A - Decrement A
            0x3D => {
                self.registers.a = self.alu_dec(self.registers.a);
                self.cycles += 4;
            }

            0x3E => {
                self.registers.a = self.fetch_byte();
                self.cycles += 8;
            }

            // LD r, r' family (0x40-0x7F) - Load register to register
            0x40..=0x7F => {
                // Special case: 0x76 is HALT (already implemented)
                if opcode == 0x76 {
                    self.halted = true;
                    self.cycles += 4;
                } else {
                    // Decode source and destination from opcode
                    let dest_idx = (opcode - 0x40) >> 3; // Upper 3 bits
                    let src_idx = (opcode - 0x40) & 0x07; // Lower 3 bits
                    
                    // Get source value (index 6 means (HL) - memory)
                    let value = match src_idx {
                        0 => self.registers.b,
                        1 => self.registers.c,
                        2 => self.registers.d,
                        3 => self.registers.e,
                        4 => self.registers.h,
                        5 => self.registers.l,
                        6 => {
                            let addr = self.registers.hl();
                            self.memory.read_byte(addr)
                        }
                        7 => self.registers.a,
                        _ => unreachable!(),
                    };
                    
                    // Set destination value (index 6 means (HL) - memory)
                    match dest_idx {
                        0 => self.registers.b = value,
                        1 => self.registers.c = value,
                        2 => self.registers.d = value,
                        3 => self.registers.e = value,
                        4 => self.registers.h = value,
                        5 => self.registers.l = value,
                        6 => {
                            let addr = self.registers.hl();
                            self.memory.write_byte(addr, value);
                        }
                        7 => self.registers.a = value,
                        _ => unreachable!(),
                    };
                    
                    // Cycles: 4 for register-to-register, 8 if memory involved
                    self.cycles += if src_idx == 6 || dest_idx == 6 { 8 } else { 4 };
                }
            }

            0x80 => {
                self.alu_add(self.registers.b);
                self.cycles += 4;
            }

            // ADD A, C
            0x81 => {
                self.alu_add(self.registers.c);
                self.cycles += 4;
            }

            // ADD A, D
            0x82 => {
                self.alu_add(self.registers.d);
                self.cycles += 4;
            }

            // ADD A, E
            0x83 => {
                self.alu_add(self.registers.e);
                self.cycles += 4;
            }

            // ADD A, H
            0x84 => {
                self.alu_add(self.registers.h);
                self.cycles += 4;
            }

            // ADD A, L
            0x85 => {
                self.alu_add(self.registers.l);
                self.cycles += 4;
            }

            // ADD A, (HL) - Add value from memory at address HL
            0x86 => {
                let address = self.registers.hl();
                let value = self.memory.read_byte(address);
                self.alu_add(value);
                self.cycles += 8;  // Memory access takes 8 cycles
            }

            // ADD A, A
            0x87 => {
                self.alu_add(self.registers.a);
                self.cycles += 4;
            }


            //...so I need to implement all 256 opcodes?

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

    fn test_register_enum_get_set() {
        let mut cpu = Cpu::new();

        // Test that the enum works as expected
        Register::B.set(&mut cpu, 0x12);
        assert_eq!(Register::B.get(&cpu), 0x12);
        assert_eq!(cpu.registers.b, 0x12);

        Register::A.set(&mut cpu, 0xFF);
        assert_eq!(Register::A.get(&cpu), 0xFF);
        assert_eq!(cpu.registers.a, 0xFF);

    }
}

#[cfg(test)]
mod instruction_tests {
    use super::*;

    // Helper function to create a CPU with a program loaded
    fn setup_cpu(program: Vec<u8>) -> Cpu {
        let mut cpu = Cpu::new();
        cpu.memory.load_rom(&program);
        cpu.pc = 0x0000; // Start from beginning for tests
        cpu
    }


    #[test]
    fn test_ld_r_n_family() {
        struct TestCase {
            opcode: u8,
            value: u8,
            register: Register,
        }

        let cases = vec![
            TestCase { opcode: 0x06, value: 0x12, register: Register::B },
            TestCase { opcode: 0x0E, value: 0x34, register: Register::C },
            TestCase { opcode: 0x16, value: 0x56, register: Register::D },
            TestCase { opcode: 0x1E, value: 0x78, register: Register::E },
            TestCase { opcode: 0x26, value: 0x9A, register: Register::H },
            TestCase { opcode: 0x2E, value: 0xBC, register: Register::L },
            TestCase { opcode: 0x3E, value: 0xDE, register: Register::A },
        ];

        for case in cases {
            let mut cpu = setup_cpu(vec![case.opcode, case.value]);
            cpu.step();
            assert_eq!(
                case.register.get(&cpu),
                case.value,
                "Failed for LD {}, 0x{:02X}",
                case.register.name(),
                case.value
            );
            assert_eq!(cpu.cycles, 8)
        }
    
    }

    #[test]
    fn test_ld_r_r_family() {
        // Test LD r, r' instructions (0x40-0x7F, except 0x76 HALT)
        
        for dest in Register::all() {
            for src in Register::all() {
                // Calculate opcode: 0x40 + (dest * 8) + src
                let dest_idx = match dest {
                    Register::B => 0,
                    Register::C => 1,
                    Register::D => 2,
                    Register::E => 3,
                    Register::H => 4,
                    Register::L => 5,
                    Register::A => 7,
                };

                let src_idx = match src {
                    Register::B => 0,
                    Register::C => 1,
                    Register::D => 2,
                    Register::E => 3,
                    Register::H => 4,
                    Register::L => 5,
                    Register::A => 7,
                };

                let opcode = 0x40 + (dest_idx * 8) + src_idx;
                let test_value = 0x42 + src_idx;

                let mut cpu = setup_cpu(vec![opcode]);

                // Set source register
                src.set(&mut cpu, test_value);

                cpu.step();

                // Check destination register
                assert_eq!(
                    dest.get(&cpu),
                    test_value,
                    "Failed for LD {}, {} (opcode 0x{:02X})",
                    dest.name(),
                    src.name(),
                    opcode
                );

                assert_eq!(cpu.cycles, 4);
            }
        }
    }

    #[test]
    fn test_inc_r_family() {
        struct TestCase {
            opcode: u8,
            register: Register,
        }

        let cases = vec![
            TestCase { opcode: 0x04, register: Register::B },
            TestCase { opcode: 0x0C, register: Register::C },
            TestCase { opcode: 0x14, register: Register::D },
            TestCase { opcode: 0x1C, register: Register::E },
            TestCase { opcode: 0x24, register: Register::H },
            TestCase { opcode: 0x2C, register: Register::L },
            TestCase { opcode: 0x3C, register: Register::A },
        ];

        for case in cases {
            // Test normal increment
            let mut cpu = setup_cpu(vec![case.opcode]);
            case.register.set(&mut cpu, 0x42);
            cpu.step();
            assert_eq!(
                case.register.get(&cpu),
                0x43,
                "Failed normal INC {}",
                case.register.name()
            );
            assert!(!cpu.registers.flag_zero());

            // Test increment with half-carry (0x0F -> 0x10)
            let mut cpu = setup_cpu(vec![case.opcode]);
            case.register.set(&mut cpu, 0x0F);
            cpu.step();
            assert_eq!(case.register.get(&cpu), 0x10);
            assert!(
                cpu.registers.flag_half_carry(),
                "Failed zero flag for INC {}",
                case.register.name()
            );

            // Test increment with zero (0xFF -> 0x00)
            let mut cpu = setup_cpu(vec![case.opcode]);
            case.register.set(&mut cpu, 0xFF);
            cpu.step();
            assert_eq!(case.register.get(&cpu), 0x00);
            assert!(
                cpu.registers.flag_zero(),
                "Failed zero flag for INC {}",
                case.register.name()
            );
        }

    }
 
    #[test]
    fn test_dec_r_family() {
        struct TestCase {
            opcode: u8,
            register: Register,
        }

        let cases = vec![
            TestCase { opcode: 0x05, register: Register::B },
            TestCase { opcode: 0x0D, register: Register::C },
            TestCase { opcode: 0x15, register: Register::D },
            TestCase { opcode: 0x1D, register: Register::E },
            TestCase { opcode: 0x25, register: Register::H },
            TestCase { opcode: 0x2D, register: Register::L },
            TestCase { opcode: 0x3D, register: Register::A },
        ];

        for case in cases {
            // Test normal increment
            let mut cpu = setup_cpu(vec![case.opcode]);
            case.register.set(&mut cpu, 0x42);
            cpu.step();
            assert_eq!(
                case.register.get(&cpu),
                0x41,
                "Failed normal DEC {}",
                case.register.name()
            );
            assert!(!cpu.registers.flag_zero());
            assert!(cpu.registers.flag_subtract());

            // Test decrement to zero
            let mut cpu = setup_cpu(vec![case.opcode]);
            case.register.set(&mut cpu, 0x01);
            cpu.step();
            assert_eq!(case.register.get(&cpu), 0x00);
            assert!(
                cpu.registers.flag_zero(),
                "Failed zero flag for DEC {}",
                case.register.name()
            );
        }

    }

    #[test]
    fn test_add_a_r_family() {
        // Test ADD with different registers (0x80-0x86)
        for (idx, reg) in Register::all()[..6].iter().enumerate() { // Skip the last one (A)
            let opcode = 0x80 + idx as u8;
            
            // Test normal addition
            let mut cpu = setup_cpu(vec![opcode]);
            Register::A.set(&mut cpu, 0x05);
            reg.set(&mut cpu, 0x03);
            cpu.step();
            assert_eq!(Register::A.get(&cpu), 0x08, "Failed ADD A,{}", reg.name());
            
            // Test with carry
            let mut cpu = setup_cpu(vec![opcode]);
            Register::A.set(&mut cpu, 0xFF);
            reg.set(&mut cpu, 0x02);
            cpu.step();
            assert_eq!(Register::A.get(&cpu), 0x01);
            assert!(cpu.registers.flag_carry(), "Failed carry for ADD A,{}", reg.name());
        }
    }
    
    #[test]
    fn test_add_a_a() {
        // Special case: ADD A, A (0x87)
        let mut cpu = setup_cpu(vec![0x87]);
        Register::A.set(&mut cpu, 0x05);
        cpu.step();
        assert_eq!(Register::A.get(&cpu), 0x0A); // 0x05 + 0x05 = 0x0A
        
        // Test with carry
        let mut cpu = setup_cpu(vec![0x87]);
        Register::A.set(&mut cpu, 0xFF);
        cpu.step();
        assert_eq!(Register::A.get(&cpu), 0xFE); // 0xFF + 0xFF = 0x1FE -> 0xFE
        assert!(cpu.registers.flag_carry());
    }
}