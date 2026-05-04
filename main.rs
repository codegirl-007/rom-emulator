pub mod bus;
pub mod cpu;
pub mod opcodes;

mod cartridge;

use std::env;
use std::fs;
use std::process;

use bus::Bus;
use cartridge::{Mirroring, Rom};
use cpu::CPU;

extern crate bitflags;
extern crate lazy_static;

fn usage(binary_name: &str) -> String {
    format!("Usage: {binary_name} <path-to-rom.nes>")
}

fn parse_rom_path() -> Result<String, String> {
    let mut args = env::args();
    let binary_name = args.next().unwrap_or_else(|| "rom-emulator".to_string());
    let rom_path = args.next().ok_or_else(|| usage(&binary_name))?;

    if args.next().is_some() {
        return Err(usage(&binary_name));
    }

    Ok(rom_path)
}

fn load_rom(path: &str) -> Result<Rom, String> {
    let raw_rom = fs::read(path).map_err(|error| format!("Failed to read ROM '{path}': {error}"))?;
    Rom::new(&raw_rom).map_err(|error| format!("Failed to parse ROM '{path}': {error}"))
}

fn validate_rom(rom: &Rom) -> Result<(), String> {
    if rom.mapper != 0 {
        return Err(format!(
            "Unsupported mapper {}. Only mapper 0 is supported right now.",
            rom.mapper
        ));
    }

    Ok(())
}

fn mirroring_name(mirroring: &Mirroring) -> &'static str {
    match mirroring {
        Mirroring::Vertical => "vertical",
        Mirroring::Horizontal => "horizontal",
        Mirroring::FourScreen => "four-screen",
    }
}

fn run() -> Result<(), String> {
    let rom_path = parse_rom_path()?;
    let rom = load_rom(&rom_path)?;
    validate_rom(&rom)?;

    println!("Loaded ROM: {rom_path}");
    println!("Mapper: {}", rom.mapper);
    println!("PRG ROM size: {} bytes", rom.prg_rom.len());
    println!("CHR ROM size: {} bytes", rom.chr_rom.len());
    println!("Mirroring: {}", mirroring_name(&rom.screen_mirroring));

    let mut cpu = CPU::new(Bus::new(rom));
    cpu.reset();
    println!("Starting execution at: {:#06x}", cpu.program_counter);
    cpu.run();

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(1);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn validate_rom_accepts_mapper_zero() {
        let rom = Rom {
            prg_rom: vec![0; 0x4000],
            chr_rom: vec![],
            mapper: 0,
            screen_mirroring: Mirroring::Horizontal,
        };

        assert!(validate_rom(&rom).is_ok());
    }

    #[test]
    fn validate_rom_rejects_other_mappers() {
        let rom = Rom {
            prg_rom: vec![0; 0x4000],
            chr_rom: vec![],
            mapper: 1,
            screen_mirroring: Mirroring::Horizontal,
        };

        assert_eq!(
            validate_rom(&rom).unwrap_err(),
            "Unsupported mapper 1. Only mapper 0 is supported right now."
        );
    }
}
