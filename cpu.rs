// This is all new to me so it's heavily commented so we can understand what is happening.

#[derive(Debug)]
#[allow(non_camel_case_types)]
// AddressingMode is a mehtod used by a CPU to determine where the operand (data or memory
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
    // Operand is at the full 16bit address (0x0000 to 0xFFFF)
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
    pub status: u8,
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
            status: 0,
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

    pub fn load_and_run(&mut self, program: Vec<u8>) {
        self.load(program);
        self.reset();
        self.run();
    }

    pub fn reset(&mut self) {
        self.register_a = 0;
        self.register_x = 0;
        self.status = 0;

        self.program_counter = self.mem_read_u16(0xFFFC);
    }

    // load the program code into memroy starting at address 0x8000. 0x8000 .. 0xFFFF is reserved
    // from program ROM and we can assume that the instructions should start somewhere here.
    pub fn load(&mut self, program: Vec<u8>) {
        self.memory[0x8000..(0x8000 + program.len())].copy_from_slice(&program[..]);
        self.mem_write_u16(0xFFFC, 0x8000);
    }

    // LDA stands for Load Accumulator. It loads a value into the accumulator register. It affects
    // the Zero Flag and Negative Flag.
    fn lda(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.register_a = value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn tax(&mut self) {
        self.register_x = self.register_a;
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn sta(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.register_a);
    }
    fn update_zero_and_negative_flags(&mut self, result: u8) {
        // update the status register
        if result == 0 {
            // 0b000_0010 represents a number where only the second bit (bit 1) is set
            // to 1 and all other bits are 0
            self.status = self.status | 0b000_0010;
        } else {
            // 0b1111_1101 represents a number where only bit 1 is 0 and the rest are 1
            self.status = self.status & 0b1111_1101;
        }

        if result & 0b1000_000 != 0 {
            self.status = self.status | 0b100_0000;
        } else {
            self.status = self.status & 0b0111_1111;
        }
    }

    // INX stands for Increment Index Register X - increase the value of the X register
    // by one and update specific processor flags based on the result.
    fn inx(&mut self) {
        self.register_x = self.register_x.wrapping_add(1);
        self.update_zero_and_negative_flags(self.register_x);
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
                // this will implement LDA (0xA9) opcode. 0xA9 is the LDA Immediate instruction in
                // the 6502 CPU.
                //
                // 0x42 tells the CPU to execute a specific operation: LDA Immediate. An opscode is
                // a command for the CPU, instructing it what to do next. Essentially, this means
                // "Load the immediate value from the next memory location into the accumulator"
                //
                // Immediate value refers to the constant value that is directly embedded in the
                // instruction itself, rather than being fetched from memory or calculated
                // directly.
                0xA9 => {
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
