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
                0xA9 => {
                    // fetch the next byte in program. This byte is the immediate value to load
                    // into the accumulator (register_a)
                    let param = program[self.program_counter as usize];
                    // Increment program counter to point to the next instruction after the
                    // parameter
                    self.program_counter += 1;
                    // store the fetch paramenter in register_a
                    self.register_a = param;

                    // now we wil update the status register
                    if self.register_a == 0 {
                        // 0b000_0010 represents a number where only the second bit (bit 1) is set
                        // to 1 and all other bits are 0
                        self.status = self.status | 0b000_0010;
                    } else {
                        // 0b1111_1101 represents a number where only bit 1 is 0 and the rest are 1
                        self.status = self.status & 0b1111_1101;
                    }
                }
                0xAA => {
                    self.register_x = self.register_a;

                    if self.register_x == 0 {
                        self.status = self.status | 0b000_0010;
                    } else {
                        self.status = self.status & 0b1111_1101;
                    }

                    if self.register_x & 0b1000_0000 != 0 {
                        self.status = self.status | 0b1000_0000;
                    } else {
                        self.status = self.status & 0b0111_1111;
                    }
                }
                0x00 => {
                    return;
                }
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
}
