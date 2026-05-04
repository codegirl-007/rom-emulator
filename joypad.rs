use std::cell::Cell;

use bitflags::bitflags;

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct JoypadButton: u8 {
        // These bits match the order the NES controller shifts out button state.
        const A = 0b0000_0001;
        const B = 0b0000_0010;
        const SELECT = 0b0000_0100;
        const START = 0b0000_1000;
        const UP = 0b0001_0000;
        const DOWN = 0b0010_0000;
        const LEFT = 0b0100_0000;
        const RIGHT = 0b1000_0000;
    }
}

pub struct Joypad {
    // `Cell` gives us interior mutability so the bus can read controller state through `&self`
    // while still advancing the serial read position like the hardware does.
    strobe: Cell<bool>,
    button_index: Cell<u8>,
    button_status: Cell<u8>,
}

impl Joypad {
    pub fn new() -> Self {
        Joypad {
            strobe: Cell::new(false),
            button_index: Cell::new(0),
            button_status: Cell::new(JoypadButton::empty().bits()),
        }
    }

    pub fn write(&self, data: u8) {
        // Bit 0 controls whether the controller keeps reporting only the A button (`1`) or shifts
        // through all buttons one by one (`0`).
        self.strobe.set(data & 1 == 1);
        if self.strobe.get() {
            self.button_index.set(0);
        }
    }

    pub fn read(&self) -> u8 {
        let button_index = self.button_index.get();
        if button_index > 7 {
            // After all 8 buttons are shifted out, NES controllers keep returning 1.
            return 1;
        }

        let response = (self.button_status.get() >> button_index) & 1;
        if !self.strobe.get() {
            // When strobe is off, each read advances to the next button bit.
            self.button_index.set(button_index + 1);
        }

        response
    }

    pub fn set_button_pressed(&self, button: JoypadButton, pressed: bool) {
        // Rebuild the bitflag value, toggle the requested button, then store the raw bits back.
        let mut button_status = JoypadButton::from_bits_truncate(self.button_status.get());
        button_status.set(button, pressed);
        self.button_status.set(button_status.bits());
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn strobe_mode_repeats_a_button_state() {
        let joypad = Joypad::new();
        joypad.set_button_pressed(JoypadButton::A, true);
        joypad.write(1);

        assert_eq!(joypad.read(), 1);
        assert_eq!(joypad.read(), 1);
        assert_eq!(joypad.read(), 1);
    }

    #[test]
    fn reads_buttons_in_standard_nes_order() {
        let joypad = Joypad::new();
        joypad.set_button_pressed(JoypadButton::A, true);
        joypad.set_button_pressed(JoypadButton::SELECT, true);
        joypad.set_button_pressed(JoypadButton::UP, true);
        joypad.set_button_pressed(JoypadButton::LEFT, true);

        joypad.write(0);

        assert_eq!(joypad.read(), 1);
        assert_eq!(joypad.read(), 0);
        assert_eq!(joypad.read(), 1);
        assert_eq!(joypad.read(), 0);
        assert_eq!(joypad.read(), 1);
        assert_eq!(joypad.read(), 0);
        assert_eq!(joypad.read(), 1);
        assert_eq!(joypad.read(), 0);
    }

    #[test]
    fn reads_return_one_after_all_buttons_are_shifted_out() {
        let joypad = Joypad::new();
        joypad.write(0);

        for _ in 0..8 {
            joypad.read();
        }

        assert_eq!(joypad.read(), 1);
        assert_eq!(joypad.read(), 1);
    }
}
