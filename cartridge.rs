const NES_TAG: [u8; 4] = [0x4E, 0x45, 0x53, 0x1A];
const PRG_ROM_PAGE_SIZE: usize = 16384;
const CHR_ROM_PAGE_SIZE: usize = 8192;

#[derive(Clone, Debug, PartialEq)]
pub enum Mirroring {
    Vertical,
    Horizontal,
    FourScreen,
}

/// Contains all information extracted from the NES ROM file
pub struct Rom {
    // Contains the Program ROM which is the main executable
    pub prg_rom: Vec<u8>,
    // Contains the Character ROM which is used for graphics
    pub chr_rom: Vec<u8>,
    // Indicates which memory mapper to use
    pub mapper: u8,
    // Determines if the background tiles are mirrored across the screen
    pub screen_mirroring: Mirroring,
}

impl Rom {
    pub fn new(raw: &Vec<u8>) -> Result<Rom, String> {
        if raw.len() < 16 {
            return Err("ROM is too small to contain a valid iNES header".to_string());
        }

        // Check for valid header. Compare the first 4 bytes to NES_TAG
        // We want to ensure that we get the standard iNES format or else we want to error out.
        if &raw[0..4] != NES_TAG {
            return Err("File is not in iNES file format".to_string());
        }

        // Extract the mapper number. The mapper dictates how to interpret the memory layout of the
        // ROM. If we don't have this, we won't know how tto access the PRG (program) or the CHR
        // (character) data correctly
        let mapper = (raw[7] & 0b1111_0000) | (raw[6] >> 4);

        // Check iNES version. We want iNES 1.0 because iNES 2.0 introduces additional
        // complexities.
        let ines_ver = (raw[7] >> 2) & 0b11;
        if ines_ver != 0 {
            return Err("NES2.0 format is not supported".to_string());
        }

        // Determine screen mirroring. This will be used by the PPC to correctly render the game
        // graphics.
        let four_screen = raw[6] & 0b1000 != 0;
        let vertical_mirroring = raw[6] & 0b1 != 0;
        let screen_mirroring = match (four_screen, vertical_mirroring) {
            (true, _) => Mirroring::FourScreen,
            (false, true) => Mirroring::Vertical,
            (false, false) => Mirroring::Horizontal,
        };

        // Calculate PRG and CHR ROM Sizes. The emulator needs to know how much data ot read and
        // where to the PRG and CHR ROMs start. This directly influences how the emulator
        // initializes memory.
        let prg_rom_size = raw[4] as usize * PRG_ROM_PAGE_SIZE;
        let chr_rom_size = raw[5] as usize * CHR_ROM_PAGE_SIZE;

        // Check if the ROM includes trainer data and adjust the starting position of the PRG.
        // Trainer data is included typically with patches or enabling cheats.
        let skip_trainer = raw[6] & 0b100 != 0;

        // We need to skip over the trainer data (if included) so that we know where the PRG data
        // starts.
        let prg_rom_start = 16 + if skip_trainer { 512 } else { 0 };
        let chr_rom_start = prg_rom_start + prg_rom_size;
        let rom_end = chr_rom_start + chr_rom_size;

        if raw.len() < rom_end {
            return Err("ROM file is truncated".to_string());
        }

        Ok(Rom {
            prg_rom: raw[prg_rom_start..(prg_rom_start + prg_rom_size)].to_vec(),
            chr_rom: raw[chr_rom_start..(chr_rom_start + chr_rom_size)].to_vec(),
            mapper,
            screen_mirroring,
        })
    }
}

pub mod test {

    use super::*;

    // we're going to simulate the rom file in memory
    struct TestRom {
        header: Vec<u8>,
        trainer: Option<Vec<u8>>,
        pgp_rom: Vec<u8>,
        chr_rom: Vec<u8>,
    }

    /// Combines the header, trainer (if any), PRG ROM, and CHR ROM into a complete ROM file.
    fn create_rom(rom: TestRom) -> Vec<u8> {
        let mut result = Vec::with_capacity(
            rom.header.len()
                + rom.trainer.as_ref().map_or(0, |t| t.len())
                + rom.pgp_rom.len()
                + rom.chr_rom.len(),
        );

        result.extend(&rom.header);
        if let Some(t) = rom.trainer {
            result.extend(t);
        }
        result.extend(&rom.pgp_rom);
        result.extend(&rom.chr_rom);

        result
    }

    /// Create a test ROM with default settings and passes it to Rom::new
    pub fn test_rom(program: Vec<u8>) -> Rom {
        let mut pgp_rom_contents = program;
        pgp_rom_contents.resize(2 * PRG_ROM_PAGE_SIZE, 0);

        let test_rom = create_rom(TestRom {
            header: vec![
                0x4E, 0x45, 0x53, 0x1A, 0x02, 0x01, 0x31, 00, 00, 00, 00, 00, 00, 00, 00, 00,
            ],
            trainer: None,
            pgp_rom: pgp_rom_contents,
            chr_rom: vec![2; 1 * CHR_ROM_PAGE_SIZE],
        });

        Rom::new(&test_rom).unwrap()
    }

    /// Test a basic rom with no trainer data
    #[test]
    fn test() {
        // Arrange and create a valid rom
        let test_rom = create_rom(TestRom {
            header: vec![
                0x4E, 0x45, 0x53, 0x1A, 0x02, 0x01, 0x31, 00, 00, 00, 00, 00, 00, 00, 00, 00,
            ],
            trainer: None,
            pgp_rom: vec![1; 2 * PRG_ROM_PAGE_SIZE],
            chr_rom: vec![2; 1 * CHR_ROM_PAGE_SIZE],
        });

        // Parse out the ROM
        let rom: Rom = Rom::new(&test_rom).unwrap();

        assert_eq!(
            rom.chr_rom,
            vec![2; 1 * CHR_ROM_PAGE_SIZE],
            "CHR ROM data mismatch"
        );
        assert_eq!(
            rom.prg_rom,
            vec![1; 2 * PRG_ROM_PAGE_SIZE],
            "PRG ROM data mismatch"
        );
        assert_eq!(rom.mapper, 3, "Mapper value mismatch");
        assert_eq!(
            rom.screen_mirroring,
            Mirroring::Vertical,
            "Screen mirroring mismatch"
        );
    }

    /// Parse ROM with trainer data
    #[test]
    fn test_with_trainer() {
        // Create a valid ROM with 512-byte trainer data
        let test_rom = create_rom(TestRom {
            header: vec![
                0x4E,
                0x45,
                0x53,
                0x1A,
                0x02,
                0x01,
                0x31 | 0b100, // this is the trainer bit set
                00,
                00,
                00,
                00,
                00,
                00,
                00,
                00,
                00,
            ],
            trainer: Some(vec![0; 512]),
            pgp_rom: vec![1; 2 * PRG_ROM_PAGE_SIZE],
            chr_rom: vec![2; 1 * CHR_ROM_PAGE_SIZE],
        });

        // Parse the ROM
        let rom: Rom = Rom::new(&test_rom).unwrap();

        assert_eq!(
            rom.chr_rom,
            vec!(2; 1 * CHR_ROM_PAGE_SIZE),
            "CHR ROM data mismatch"
        );
        assert_eq!(
            rom.prg_rom,
            vec!(1; 2 * PRG_ROM_PAGE_SIZE),
            "PRG ROM data mismatch"
        );
        assert_eq!(rom.mapper, 3, "Mapper value mismatch");
        assert_eq!(
            rom.screen_mirroring,
            Mirroring::Vertical,
            "Screen mirroring mode mismatch"
        );
    }

    /// Test invalid NES 2.0 format
    #[test]
    fn test_nes2_is_not_supported() {
        // Create ROM with NES 2.0 version flag
        let test_rom = create_rom(TestRom {
            header: vec![
                0x4E, 0x45, 0x53, 0x1A, 0x01, 0x01, 0x31, 0x8, 00, 00, 00, 00, 00, 00, 00, 00,
            ],
            trainer: None,
            pgp_rom: vec![1; 1 * PRG_ROM_PAGE_SIZE],
            chr_rom: vec![2; 1 * CHR_ROM_PAGE_SIZE],
        });

        // Try to parse the ROM
        let rom = Rom::new(&test_rom);
        match rom {
            Result::Ok(_) => assert!(false, "should not load rom"),
            Result::Err(str) => assert_eq!(str, "NES2.0 format is not supported"),
        }
    }
}
