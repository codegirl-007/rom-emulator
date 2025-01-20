// This is all new to me so it's heavily commented so we can understand what is happening.
use bitflags::bitflags;

bitflags! {
    pub struct CpuFlags: u8 {
        // Represents whether the last operation caused a carry or borrow
        const CARRY = 0b00000001;
        // Set if the result of the last operation was zero
        const ZERO = 0b00000010;
        // Disables interrupts when set
        const INTERRUPT_DISABLE = 0b00000100;
        // Enables binary-coded decimal arithmetic
        const DECIMAL_MODE = 0b00001000;
        // Indicates the CPU is handling in interupt
        const BREAK = 0b00010000;
        // A duplicate "break" flag, as both bit 4 and 5 are set by the BRK instruction
        // Bit 5 is not explicitly used on NES
        const BREAK2 = 0b00100000;
        // Set if an arithmetic operation caused an overflow (adding two positive numbers results
        // in a negative value)
        const OVERFLOW = 0b01000000;
        // Indicates whether the result of the last operation was negative - determined by the MSB
        // of the result
        const NEGATIVE = 0b10000000;
    }
}

const STACK: u16 = 0x0100;
const STACK_RESET: u8 = 0xfd;

#[derive(Debug)]
#[allow(non_camel_case_types)]
// AddressingMode is a method used by a CPU to determine where the operand (data or memory
// location) for an instruction comes from. It basically defines how the CPU finds the data it
// needs to execute a given instruction.
//
// The zero page term refers to the first 256 bytes (from 0x0000 to 0x00FF). It's called zero page
// because the high page is always zero (e.g. 00XX). Instructions in this range use fewer bytes and
// cycles compared to acessing memory in other regions. This is due to the fact that only the low
// bytes need to be specified which makes the instruction shorter and faster.
pub enum AddressingMode {
    // Operand is provided directly as part of the instruction
    Immediate,
    //Operand is located in the zero page (memory address 0x00 to 0xFF)
    ZeroPage,
    // Operand is in the zero page, with an offet from the X register
    ZeroPage_X,
    // Operand is in the zero page, with an offet from the Y register
    ZeroPage_Y,
    // Operand is at the full 16-bit address (0x0000 to 0xFFFF)
    Absolute,
    // Operand is at an absolute address, with an offset from the X register
    Absolute_X,
    // Operand is at an absolute address, with an offset from the Y register
    Absolute_Y,
    // Operand is at an address stored in a zero-page address plus an X offset
    Indirect_X,
    // Operand is at an address stored in a zero-page address plus an Y offset
    Indirect_Y,
    // No addressing mode (used for instructions that don't require an operand like NOP)
    NoneAddressing,
}

pub struct CPU {
    // The accumulator register is a specific register used for arithmetic and logic operations.
    // The cpu instruction loads a value into the accumulator register and then updates certain
    // flags in the processor status register to relect the operation of the result.
    pub register_a: u8,
    // The Process Status Register is a collection of individual bits (flags) that represent the
    // current state of the CPU. Each bit has purpose such as if a calculation resulted in zero or
    // if the result is negative
    //
    // Zero flag:
    // - Bit 1 of the status register
    // - Set if register_a == 0, cleared otherwise
    //
    // Negative flag (N) - indicates whether the result of the most recent operation is negative
    // - Bit 7 of the status register (most significant bit is Bit 7 because it's zero based)
    // - Set if the most significant bit of register_a is 1, cleared otherwise
    pub status: CpuFlags,
    // track our current position in the program
    pub program_counter: u16,
    // most commonly used to hold counters or offsets for accessing memory. The value can be loaded
    // and saved in memory, compared with values held in memory or incremented and decremented.
    //
    // X register has one special function. It can be used to copy a stack pointer or change it's
    // value.
    pub register_x: u8,
    pub register_y: u8,
    // this creates an array with size of 0xFFFF and initializes all elements to 0 (see below in t
    // he new function). 0xFFFF is hexadecimal for 65535 in decimal. This defines the size of the
    // array with 65535 elements, the typical size for a 6502 CPU system.
    memory: [u8; 0xFFFF],
}

trait Mem {
    fn mem_read(&self, addr: u16) -> u8;

    fn mem_write(&mut self, addr: u16, data: u8);

    // combines two consecutives bytes in memory into a single 16 bit value
    fn mem_read_u16(&mut self, pos: u16) -> u16 {
        // read the low byte (pos)
        let lo = self.mem_read(pos) as u16;
        // read the high byte (pos + 1)
        let hi = self.mem_read(pos + 1) as u16;
        // combine the bytes. Shelf the high byte 8 bits to the left, effectively moving it into
        // the upper 8 bits of a 16-bit value. This will combine to a single 16 bit value.
        (hi << 8) | (lo as u16)
    }

    // split a 16-bit number into 2 8-bit pieces
    fn mem_write_u16(&mut self, pos: u16, data: u16) {
        // take the top half of the 16 bit number
        let hi = (data >> 8) as u8;
        // take the bottom half of the 16 bit number
        let lo = (data & 0xff) as u8;
        // store low byte in the first compartment of memory (pos)
        self.mem_write(pos, lo);
        // store the high byte in the next compartment (pos + 1)
        self.mem_write(pos + 1, hi)
    }
}

impl Mem for CPU {
    fn mem_read(&self, addr: u16) -> u8 {
        self.memory[addr as usize]
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        self.memory[addr as usize] = data;
    }
}

impl CPU {
    pub fn new() -> Self {
        CPU {
            register_a: 0,
            status: CpuFlags::from_bits_truncate(0b100100),
            program_counter: 0,
            register_x: 0,
            register_y: 0,
            memory: [0; 0xFFFF],
        }
    }

    /// Determines the memory address for the operand based on the specified addressing mode.
    ///
    /// # Arguments
    /// - `mode`: The addressing mode, which speicifies how the oeprand is accessed.
    ///
    /// # Returns
    /// - A 16 bit memory address where the operand can be found
    fn get_operand_address(&mut self, mode: &AddressingMode) -> u16 {
        match mode {
            // Immediate mode: The operand is part of the instruction itself, located at the
            // program counter
            AddressingMode::Immediate => self.program_counter,

            // ZeroPage mode: The operand is in the zero page (first 256 bytes of memory), with the
            // address specificed as a single byte at the program counter.
            AddressingMode::ZeroPage => self.mem_read(self.program_counter) as u16,

            // Absolute mode: The operand's address is a full 16-bit address, stored in two
            // consecutive bytes starting at the program counter
            AddressingMode::Absolute => self.mem_read_u16(self.program_counter),

            // ZeroPage_X mode: The operand is in the zero page, with it's address calculated by
            // adding the X register to a base address stored at the program counter
            AddressingMode::ZeroPage_X => {
                let pos = self.mem_read(self.program_counter);
                let addr = pos.wrapping_add(self.register_x) as u16;
                addr
            }

            // ZeroPage_Y mode: Similar to ZeroPage_X, but the address is calculated
            // using the Y register instead of the X register.
            AddressingMode::ZeroPage_Y => {
                let pos = self.mem_read(self.program_counter);
                let addr = pos.wrapping_add(self.register_y) as u16;
                addr
            }

            // Absolute_X mode: The operand's address is calculated by adding the X register
            // to a 16-bit base address stored at the program counter.
            AddressingMode::Absolute_X => {
                let base = self.mem_read_u16(self.program_counter);
                let addr = base.wrapping_add(self.register_x as u16);
                addr
            }

            // Absolute_Y mode: Similar to Absolute_X, but the address is calculated
            // by adding the Y register to the 16-bit base address.
            AddressingMode::Absolute_Y => {
                let base = self.mem_read_u16(self.program_counter);
                let addr = base.wrapping_add(self.register_x as u16);
                addr
            }

            // Indirect_X mode: The operand's address is calculated by first adding the X register
            // to a zero-page base address, then fetching the actual 16-bit address from
            // the zero page.
            AddressingMode::Indirect_X => {
                // read the single byte from the program counter. This is our base.
                let base = self.mem_read(self.program_counter);
                // Take the base and add the value in the X register. If the result goes passed
                // 0xFF, it wraps back around to 0x00 (like starting over in a circle). This is
                // essentially behaving like a ring buffer. Using wrapping_add ensures the
                // calculation stays within the valid range without requiring additional checks
                let ptr: u8 = (base as u8).wrapping_add(self.register_x);
                // Read the low byte of the memory address from memory at ptr. The goal here is
                // to fetch the 16 byte address from the zero page using the pointer.
                let lo = self.mem_read(ptr as u16);
                // read the high byte of the memory address from memory at ptr + 1
                let hi = self.mem_read(ptr.wrapping_add(1) as u16);
                // take the high byte and shift it left by 8 bits to place it in the upper half of
                // the 16 bit value. Then take the low byte and keep it in the lower half of the 16
                // bit value. Combine the two values using the bitwise OR (|) operator to form a
                // full 16 bit value.
                (hi as u16) << 8 | (lo as u16)
            }

            // Indirect_Y mode: The operand's address is calculated by first fetching a 16-bit address
            // from a zero-page base address, then adding the Y register to it.
            AddressingMode::Indirect_Y => {
                // read from the program counter (an 8 bit value). This pointer specifies a zero
                // page address where the actual 16 bit address is stored.
                let base = self.mem_read(self.program_counter);
                // read the low byte of the final address from the zero page address specificed by
                // `base`
                let lo = self.mem_read(base as u16);
                // read the high byte of the final address from the next consecutive zero-page
                // address (base +1). wrapping_add(1) ensures that if base = 0xFF, it wraps around
                // the 0x00
                let hi = self.mem_read((base as u8).wrapping_add(1) as u16);
                // Combine the low and high bytes into a 16 bit address
                let deref_base = (hi as u16) << 8 | (lo as u16);
                // Add the Y Registered to the dereferenced address
                let deref = deref_base.wrapping_add(self.register_y as u16);
                // Return the final address
                deref
            }

            // NoneAddressing: This mode is used for instructions that don't require an operand
            // or don't involve memory access.
            AddressingMode::NoneAddressing => {
                panic!("mode {:?} is not supported", mode);
            }
        }
    }

    /// Road the program, reset the state of the cpu and then run the program.
    pub fn load_and_run(&mut self, program: Vec<u8>) {
        self.load(program);
        self.reset();
        self.run();
    }

    /// Reset the state of the CPU
    pub fn reset(&mut self) {
        self.register_a = 0;
        self.register_x = 0;
        self.status = CpuFlags::from_bits_truncate(0b100100);

        self.program_counter = self.mem_read_u16(0xFFFC);
    }

    /// Load the program code into memroy starting at address 0x8000. 0x8000 .. 0xFFFF is reserved
    /// from program ROM and we can assume that the instructions should start somewhere here.
    pub fn load(&mut self, program: Vec<u8>) {
        self.memory[0x8000..(0x8000 + program.len())].copy_from_slice(&program[..]);
        self.mem_write_u16(0xFFFC, 0x8000);
    }

    /// LDA stands for Load Accumulator. It loads a value into the accumulator register. It affects
    /// the Zero Flag and Negative Flag.
    fn lda(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.register_a = value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    /// TAX instruction copies the value from teh accumulator register (register_a) into the x
    /// register. The way this works is that the CPU takes the current value in the A register,
    /// copies it to the X register, updates the Zero Flag and Negative Flag in the status register.
    ///
    /// The Zero Flag is set if the value is copied to the X register is 0
    /// The Negative Flag is set if the most significant bit of the vlaue (Bit 7) is 1
    ///
    /// In the TAX instruction, we don't need any arguments because it always stransfers data from A
    /// to X.
    fn tax(&mut self) {
        self.register_x = self.register_a;
        self.update_zero_and_negative_flags(self.register_x);
    }

    /// STA stands for Store Accumulator. This instruction takes the value in the accumulator
    /// register (register_a) and stores it in a specified memory location.
    ///
    /// How this works is that the CPU reads the value currently in the accumulator (register_a)
    /// The CPu then writes this value to the specified memory address.
    fn sta(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.register_a);
    }

    /// Perform a bitwise AND operation betwen the value in register_a and a value fetched from
    /// memory (determined by the addressing mode)
    ///
    /// The result is stored back in register_a. This is useful for isolating specific bits in the
    /// accumulator.
    fn and(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let data = self.mem_read(addr);

        self.set_register_a(data & self.register_a);
    }

    /// This is used to update the status register flags based on the operation. It updates the Zero
    /// Flag and the Negative Flag.
    ///
    /// This will ensure that the CPU's status register reflects whether the result is zerl (update
    /// the Zero Flag) and if the result is negative (update the Negative Flag)
    fn update_zero_and_negative_flags(&mut self, result: u8) {
        if result == 0 {
            self.status.insert(CpuFlags::ZERO)
        } else {
            self.status.remove(CpuFlags::ZERO);
        }

        if result & 0b1000_000 != 0 {
            self.status.insert(CpuFlags::NEGATIVE)
        } else {
            self.status.remove(CpuFlags::NEGATIVE);
        }
    }

    /// INX stands for Increment Index Register X - increase the value of the X register
    /// by one and update specific processor flags based on the result.
    fn inx(&mut self) {
        self.register_x = self.register_x.wrapping_add(1);
        self.update_zero_and_negative_flags(self.register_x);
    }

    /// Simulate the ADC (Add with Carry) instruction. It adds a value (data) to the accumulator
    /// register (register_a), optionally including the carry flag and updates the status falgs
    /// accordingly.
    fn add_to_register_a(&mut self, data: u8) {
        // Add register_a, data and carry flag togehter. All values are cast to u16 to prevent
        // overflow during the calculation (since the result might exceed 8 bits).
        let sum = self.register_a as u16
            + data as u16
            + (if self.status.contains(CpuFlags::CARRY) {
                1
            } else {
                0
            }) as u16;

        let carry = sum > 0xff;

        // the carry flag is set if the result exceeds 255 (0xFF), meaning the additon produced a
        // value that requires more than 8 bits to store.
        if carry {
            // this happens if the sum = 0x101
            self.status.insert(CpuFlags::CARRY);
        } else {
            // this happens if sum = 0xFE
            self.status.remove(CpuFlags::CARRY);
        }

        // reduce the sum down to a u8 which will discord any overflow beyond 8 bits
        let result = sum as u8;

        // update the overflow flag to be set if there is a signed arithmetic overflow.
        // A signed arithmetic overflow occurs when adding two signed numbers produces a result
        // that is outside th range of a signed 8-bit value (-128 to 127)
        //
        // `data ^ result` checks if the sign of data is different from the sign of result
        // `result ^ self.register_a` checks if the sign of result is different from the sign of
        // register_a.
        // `& 0x80` isolates the most significant bit whcih indicates the sign in signed arithmetic.
        if (data ^ result) & (result ^ self.register_a) & 0x80 != 0 {
            self.status.insert(CpuFlags::OVERFLOW)
        } else {
            self.status.remove(CpuFlags::OVERFLOW);
        }

        // store the final 8-bit result back in the accumulator
        self.set_register_a(result);
    }

    /// This will set the value of the A register (register_A)
    fn set_register_a(&mut self, value: u8) {
        self.register_a = value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    /// SBC stands for Subtract with Borrow. It is the counter to ADC (Add with Carry) instruction
    /// and performs subtraction while considering the Carry Flag. This works with unsigned binary
    /// numbers or two's complement signed numbers.
    ///
    /// The Carry flag represents the absence of a borrow.
    /// The Overflow flag is set if the result goes outside the range of a signed 8-bit value
    /// The Negative flag and Zero flag are updated based on the result
    fn sbc(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let data = self.mem_read(addr);
        self.add_to_register_a(((data as i8).wrapping_neg().wrapping_sub(1)) as u8);
    }

    /// ADC stands for Add with Carry
    fn adc(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.add_to_register_a(value);
    }

    // The interpret method takes in mutalbe reference to self as we know we will need to modify
    // register_a during execution
    //
    // - Fetch next instruction from instruction memory
    // - Decode instruction
    // - Execute the instruction
    // - Repeat
    pub fn run(&mut self) {
        // We need an infinite loop to continuously fetch instructions from the program array. We
        // use the program_counter to keep track fo the current instruction.
        loop {
            // set the opscode to the current byte in the program at the address indicated by
            // program counter
            let opscode = self.mem_read(self.program_counter);
            // we increment program counterto point to the next byte
            self.program_counter += 1;

            match opscode {
                // This will implement LDA (0xA9) opcode. 0xA9 is the LDA Immediate instruction in
                // the 6502 CPU.
                //
                // 0x42 tells the CPU to execute a specific operation: LDA Immediate. An opscode is
                // a command for the CPU, instructing it what to do next. Essentially, this means
                // "Load the immediate value from the next memory location into the accumulator"
                //
                // Immediate value refers to the constant value that is directly embedded in the
                // instruction itself, rather than being fetched from memory or calculated
                // directly.
                0xA9 | 0xa5 | 0xb4 | 0xad | 0xbd | 0xb9 | 0xa1 | 0xb1 => {
                    self.lda(&AddressingMode::Immediate);
                    self.program_counter += 1;
                }
                0xA5 => {
                    self.lda(&AddressingMode::ZeroPage);
                    self.program_counter += 1;
                }
                0xAD => {
                    self.lda(&AddressingMode::Absolute);
                    self.program_counter += 2;
                }
                0x85 => {
                    self.sta(&AddressingMode::ZeroPage);
                    self.program_counter += 1;
                }
                0x95 => {
                    self.sta(&AddressingMode::ZeroPage);
                    self.program_counter += 1;
                }
                // implement the 0xAA opcode which corresponds to the TAX (transfer acculumater to
                // X register)
                0xAA => self.tax(),
                // INX stands for Increment Index Register X - increase the value of the X register
                // by one and update specific processor flags based on the result.
                0xe8 => self.inx(),
                0x00 => return,
                _ => todo!(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lda_sets_register_a() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x05, 0x00]);
        assert_eq!(cpu.register_a, 0x05);
        assert!(cpu.status & 0b0000_0010 == 0b00);
        assert!(cpu.status & 0b1000_0000 == 0);
    }

    #[test]
    fn test_0xa9_lda_zero_flag() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x00, 0x00]);
        assert!(cpu.status & 0b0000_0010 == 0b10);
    }

    #[test]
    fn test_0xaa_tax_move_to_a_to_x() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x0A, 0xaa, 0x00]);

        assert_eq!(cpu.register_x, 10)
    }

    #[test]
    fn test_5_ops_working_together() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0xc0, 0xaa, 0xe8, 0x00]);

        assert_eq!(cpu.register_x, 0xc1)
    }

    #[test]
    fn test_inx_overflow() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0xff, 0xaa, 0xe8, 0xe8, 0x00]);

        assert_eq!(cpu.register_x, 1)
    }
}
