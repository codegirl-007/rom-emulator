use std::cell::RefCell;

use crate::{cartridge::Rom, cpu::Mem, joypad::{Joypad, JoypadButton}, ppu::PPU};

const RAM: u16 = 0x0000;
const RAM_MIRRORS_END: u16 = 0x1FFF;
const PPU_REGISTERS: u16 = 0x2000;
const PPU_REGISTERS_MIRRORS_END: u16 = 0x3FFF;
// NES controllers are memory-mapped, so the CPU reads and writes them through these addresses.
const JOYPAD_1: u16 = 0x4016;
const JOYPAD_2: u16 = 0x4017;

pub struct Bus {
    cpu_vram: [u8; 2048],
    rom: Rom,
    // Keep both controller slots in the bus so CPU input works like any other hardware register.
    joypad1: Joypad,
    joypad2: Joypad,
    ppu: RefCell<PPU>,
}

impl Bus {
    pub fn new(rom: Rom) -> Self {
        let ppu = PPU::new(rom.chr_rom.clone(), rom.screen_mirroring.clone());

        Bus {
            cpu_vram: [0; 2048],
            rom,
            joypad1: Joypad::new(),
            joypad2: Joypad::new(),
            ppu: RefCell::new(ppu),
        }
    }

    fn read_prg_rom(&self, mut addr: u16) -> u8 {
        addr -= 0x8000;
        if self.rom.prg_rom.len() == 0x4000 && addr >= 0x4000 {
            addr = addr % 0x4000;
        }
        self.rom.prg_rom[addr as usize]
    }

    pub fn set_button_pressed(&mut self, button: JoypadButton, pressed: bool) {
        // For now only player 1 input is wired to the keyboard.
        self.joypad1.set_button_pressed(button, pressed);
    }
}

impl Mem for Bus {
    fn mem_read(&self, addr: u16) -> u8 {
        match addr {
            RAM..=RAM_MIRRORS_END => {
                let mirror_down_addr = addr & 0b00000111_11111111;
                self.cpu_vram[mirror_down_addr as usize]
            }
            PPU_REGISTERS..=PPU_REGISTERS_MIRRORS_END => {
                let mut ppu = self.ppu.borrow_mut();
                match 0x2000 + (addr - 0x2000) % 8 {
                    0x2002 => ppu.read_status(),
                    0x2004 => ppu.read_oam_data(),
                    0x2007 => ppu.read_data(),
                    _ => 0,
                }
            }
            // Controller reads are serial: each call returns the next button bit.
            JOYPAD_1 => self.joypad1.read(),
            JOYPAD_2 => self.joypad2.read(),
            0x8000..=0xFFFF => self.read_prg_rom(addr),

            _ => {
                println!("Ignoring mem access at {}", addr);
                0
            }
        }
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        match addr {
            RAM..=RAM_MIRRORS_END => {
                let mirror_down_addr = addr & 0b11111111111;
                self.cpu_vram[mirror_down_addr as usize] = data;
            }
            PPU_REGISTERS..=PPU_REGISTERS_MIRRORS_END => {
                let mut ppu = self.ppu.borrow_mut();
                match 0x2000 + (addr - 0x2000) % 8 {
                    0x2000 => ppu.write_to_ctrl(data),
                    0x2001 => ppu.write_to_mask(data),
                    0x2003 => ppu.write_to_oam_addr(data),
                    0x2004 => ppu.write_to_oam_data(data),
                    0x2005 => ppu.write_to_scroll(data),
                    0x2006 => ppu.write_to_ppu_addr(data),
                    0x2007 => ppu.write_to_data(data),
                    _ => {}
                }
            }
            JOYPAD_1 => {
                // Writing to 0x4016 updates controller strobe state. Real NES hardware uses the
                // same write for both controller ports, so we mirror it here too.
                self.joypad1.write(data);
                self.joypad2.write(data);
            }
            JOYPAD_2 => {}
            0x8000..=0xFFFF => {
                panic!("Attempt to write to ROM space")
            }
            _ => {
                println!("Ignoring mem write-access at {}", addr);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::cartridge::test;

    #[test]
    fn test_mem_read_write_to_ram() {
        let mut bus = Bus::new(test::test_rom(vec![]));
        bus.mem_write(0x01, 0x55);
        assert_eq!(bus.mem_read(0x01), 0x55);
    }

    #[test]
    fn test_joypad_one_is_mapped_to_standard_addresses() {
        let mut bus = Bus::new(test::test_rom(vec![]));
        bus.set_button_pressed(JoypadButton::A, true);
        bus.set_button_pressed(JoypadButton::START, true);
        bus.mem_write(JOYPAD_1, 1);
        bus.mem_write(JOYPAD_1, 0);

        assert_eq!(bus.mem_read(JOYPAD_1), 1);
        assert_eq!(bus.mem_read(JOYPAD_1), 0);
        assert_eq!(bus.mem_read(JOYPAD_1), 0);
        assert_eq!(bus.mem_read(JOYPAD_1), 1);
    }

    #[test]
    fn test_ppu_registers_are_mirrored_every_eight_bytes() {
        let mut bus = Bus::new(test::test_rom(vec![]));
        bus.mem_write(0x200E, 0x20);
        bus.mem_write(0x200E, 0x00);
        bus.mem_write(0x200F, 0x33);
        bus.mem_write(0x200E, 0x20);
        bus.mem_write(0x200E, 0x00);
        bus.mem_read(0x2007);

        assert_eq!(bus.mem_read(0x3FFF), 0x33);
    }
}
