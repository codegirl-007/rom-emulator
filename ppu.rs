use crate::cartridge::Mirroring;

pub struct PPU {
    // Pattern table graphics come from CHR ROM on mapper-0 cartridges.
    chr_rom: Vec<u8>,
    // The NES has 2KB of VRAM for nametables, mirrored depending on cartridge wiring.
    vram: [u8; 2048],
    palette_table: [u8; 32],
    oam_data: [u8; 256],
    mirroring: Mirroring,
    ctrl: u8,
    mask: u8,
    status: u8,
    oam_addr: u8,
    ppu_addr: u16,
    addr_latch: bool,
    scroll_latch: bool,
    scroll_x: u8,
    scroll_y: u8,
    internal_data_buf: u8,
}

impl PPU {
    pub fn new(chr_rom: Vec<u8>, mirroring: Mirroring) -> Self {
        PPU {
            chr_rom,
            vram: [0; 2048],
            palette_table: [0; 32],
            oam_data: [0; 256],
            mirroring,
            ctrl: 0,
            mask: 0,
            status: 0,
            oam_addr: 0,
            ppu_addr: 0,
            addr_latch: false,
            scroll_latch: false,
            scroll_x: 0,
            scroll_y: 0,
            internal_data_buf: 0,
        }
    }

    pub fn write_to_ctrl(&mut self, data: u8) { self.ctrl = data; }
    pub fn write_to_mask(&mut self, data: u8) { self.mask = data; }
    pub fn write_to_oam_addr(&mut self, data: u8) { self.oam_addr = data; }
    pub fn read_oam_data(&self) -> u8 { self.oam_data[self.oam_addr as usize] }

    pub fn write_to_oam_data(&mut self, data: u8) {
        self.oam_data[self.oam_addr as usize] = data;
        self.oam_addr = self.oam_addr.wrapping_add(1);
    }

    pub fn read_status(&mut self) -> u8 {
        // Reading PPUSTATUS clears the two-write latches used by PPUSCROLL and PPUADDR.
        let status = self.status;
        self.addr_latch = false;
        self.scroll_latch = false;
        self.status &= 0b0111_1111;
        status
    }

    pub fn write_to_scroll(&mut self, data: u8) {
        if !self.scroll_latch {
            self.scroll_x = data;
        } else {
            self.scroll_y = data;
        }
        self.scroll_latch = !self.scroll_latch;
    }

    pub fn write_to_ppu_addr(&mut self, data: u8) {
        if !self.addr_latch {
            self.ppu_addr = ((data as u16) << 8) | (self.ppu_addr & 0x00FF);
        } else {
            self.ppu_addr = (self.ppu_addr & 0xFF00) | data as u16;
        }
        self.ppu_addr &= 0x3FFF;
        self.addr_latch = !self.addr_latch;
    }

    pub fn read_data(&mut self) -> u8 {
        let addr = self.ppu_addr;
        self.increment_vram_addr();

        match addr {
            0x0000..=0x1FFF => {
                let result = self.internal_data_buf;
                self.internal_data_buf = self.chr_rom[addr as usize];
                result
            }
            0x2000..=0x3EFF => {
                let result = self.internal_data_buf;
                self.internal_data_buf = self.vram[self.mirror_vram_addr(addr)];
                result
            }
            0x3F00..=0x3FFF => self.palette_table[self.mirror_palette_addr(addr)],
            _ => 0,
        }
    }

    pub fn write_to_data(&mut self, data: u8) {
        let addr = self.ppu_addr;
        match addr {
            0x0000..=0x1FFF => panic!("Attempt to write to CHR ROM space"),
            0x2000..=0x3EFF => {
                let index = self.mirror_vram_addr(addr);
                self.vram[index] = data;
            }
            0x3F00..=0x3FFF => {
                let index = self.mirror_palette_addr(addr);
                self.palette_table[index] = data;
            }
            _ => {}
        }
        self.increment_vram_addr();
    }

    fn increment_vram_addr(&mut self) {
        let increment = if self.ctrl & 0b0000_0100 != 0 { 32 } else { 1 };
        self.ppu_addr = (self.ppu_addr + increment) & 0x3FFF;
    }

    fn mirror_vram_addr(&self, addr: u16) -> usize {
        let mirrored = (addr - 0x2000) % 0x1000;
        let table = mirrored / 0x0400;
        let offset = (mirrored % 0x0400) as usize;

        match self.mirroring {
            Mirroring::Vertical => match table {
                0 | 2 => offset,
                1 | 3 => 0x0400 + offset,
                _ => unreachable!(),
            },
            Mirroring::Horizontal => match table {
                0 | 1 => offset,
                2 | 3 => 0x0400 + offset,
                _ => unreachable!(),
            },
            Mirroring::FourScreen => (mirrored as usize) % self.vram.len(),
        }
    }

    fn mirror_palette_addr(&self, addr: u16) -> usize {
        let mut index = ((addr - 0x3F00) % 32) as usize;
        if matches!(index, 0x10 | 0x14 | 0x18 | 0x1C) {
            index -= 0x10;
        }
        index
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn ppu_data_reads_buffer_chr_rom() {
        let mut ppu = PPU::new(vec![0xAA; 0x2000], Mirroring::Horizontal);
        ppu.write_to_ppu_addr(0x00);
        ppu.write_to_ppu_addr(0x10);

        assert_eq!(ppu.read_data(), 0);
        assert_eq!(ppu.read_data(), 0xAA);
    }

    #[test]
    fn horizontal_mirroring_maps_first_two_tables_together() {
        let mut ppu = PPU::new(vec![0; 0x2000], Mirroring::Horizontal);
        ppu.write_to_ppu_addr(0x20);
        ppu.write_to_ppu_addr(0x00);
        ppu.write_to_data(0x11);
        ppu.write_to_ppu_addr(0x24);
        ppu.write_to_ppu_addr(0x00);
        ppu.write_to_data(0x22);

        assert_eq!(ppu.vram[0], 0x22);
    }
}
