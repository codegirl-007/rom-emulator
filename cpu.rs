// This is all new to me so it's heavily commented so we can understand what is happening.

pub struct CPU {
    // the accumulator register is a specific register  used for arithmetic and logic operations
    // the cpu instruction loads a value into the accumulator register and then updates certain
    // flags in the processor status register to relect the operation of the result
    pub register_a: u8,
    // THe Process Status Register is a collection of individual bits (flags) that represent the
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
}

impl CPU {
    pub fn new() -> Self {
        CPU {
            register_a: 0,
            status: 0,
            program_counter: 0,
            register_x: 0,
        }
    }

    // LDA stands for Load Accumulator. It loads a value into the accumulator register. It affects
    // the Zero Flag and Negative Flag.
    fn lda(&mut self, value: u8) {
        self.register_a = value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn tax(&mut self) {
        self.register_x = self.register_a;
        self.update_zero_and_negative_flags(self.register_x);
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
    pub fn interpret(&mut self, program: Vec<u8>) {
        self.program_counter = 0;

        // We need an infinite loop to continuously fetch instructions from the program array. We
        // use the program_counter to keep track fo the current instruction.
        loop {
            // set the opscode to the current byte in the program at the address indicated by
            // program counter
            let opscode = program[self.program_counter as usize];
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
                // Immediate value refers to the constant valuethat is directly embedded in the
                // instruction itself, rather than being fetched from memory or calculated
                // directly.
                0xA9 => {
                    // fetch the next byte in program. This byte is the immediate value to load
                    // into the accumulator (register_a)
                    let param = program[self.program_counter as usize];
                    // Increment program counter to point to the next instruction after the
                    // parameter. The program counter must always point to the next instruction to
                    // be executed.
                    self.program_counter += 1;
                    // store the fetch parameter in register_a - handling the actual loading of the
                    // value into the accumulator and updates the CPU flags.
                    self.lda(param);
                }
                // implement the 0xAA opcode which corresponds to the TAX (transfer acculumater to
                // X register)
                0xAA => self.tax(),
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
        cpu.interpret(vec![0xa9, 0x05, 0x00]);
        assert_eq!(cpu.register_a, 0x05);
        assert!(cpu.status & 0b0000_0010 == 0b00);
        assert!(cpu.status & 0b1000_0000 == 0);
    }

    #[test]
    fn test_0xa9_lda_zero_flag() {
        let mut cpu = CPU::new();
        cpu.interpret(vec![0xa9, 0x00, 0x00]);
        assert!(cpu.status & 0b0000_0010 == 0b10);
    }

    #[test]
    fn test_0xaa_tax_move_to_a_to_x() {
        let mut cpu = CPU::new();
        cpu.register_a = 10;
        cpu.interpret(vec![0xaa, 0x00]);

        assert_eq!(cpu.register_x, 10)
    }

    #[test]
    fn test_5_ops_working_together() {
        let mut cpu = CPU::new();
        cpu.interpret(vec![0xa9, 0xc0, 0xaa, 0xe8, 0x00]);

        assert_eq!(cpu.register_x, 0xc1)
    }

    #[test]
    fn test_inx_overflow() {
        let mut cpu = CPU::new();
        cpu.register_x = 0xff;
        cpu.interpret(vec![0xe8, 0xe8, 0x00]);

        assert_eq!(cpu.register_x, 1)
    }
}
