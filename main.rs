pub mod bus;
pub mod cpu;
pub mod joypad;
pub mod opcodes;

mod cartridge;

use std::env;
use std::fs;
use std::mem::MaybeUninit;
use std::process;

use bus::Bus;
use cartridge::{Mirroring, Rom};
use cpu::CPU;
use joypad::JoypadButton;
use sdl2::EventPump;
use sdl2::sys;

extern crate bitflags;
extern crate lazy_static;

fn usage(binary_name: &str) -> String {
    format!("Usage: {binary_name} <path-to-rom.nes>")
}

fn parse_rom_path() -> Result<String, String> {
    // `env::args()` gives us the raw CLI arguments. We expect exactly one ROM path after the
    // program name.
    let mut args = env::args();
    let binary_name = args.next().unwrap_or_else(|| "rom-emulator".to_string());
    let rom_path = args.next().ok_or_else(|| usage(&binary_name))?;

    if args.next().is_some() {
        return Err(usage(&binary_name));
    }

    Ok(rom_path)
}

fn load_rom(path: &str) -> Result<Rom, String> {
    // Read the entire `.nes` file into memory, then let `Rom::new` parse the iNES structure.
    let raw_rom = fs::read(path).map_err(|error| format!("Failed to read ROM '{path}': {error}"))?;
    Rom::new(&raw_rom).map_err(|error| format!("Failed to parse ROM '{path}': {error}"))
}

fn validate_rom(rom: &Rom) -> Result<(), String> {
    // Step 5 still targets NROM only, so fail fast on mappers we do not implement yet.
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

fn set_button_for_key(cpu: &mut CPU, keycode: i32, pressed: bool) {
    // Translate desktop keyboard keys into NES controller buttons.
    let button = match keycode {
        value if value == sys::SDL_KeyCode::SDLK_z as i32 => Some(JoypadButton::A),
        value if value == sys::SDL_KeyCode::SDLK_x as i32 => Some(JoypadButton::B),
        value if value == sys::SDL_KeyCode::SDLK_RETURN as i32 => Some(JoypadButton::START),
        value if value == sys::SDL_KeyCode::SDLK_RSHIFT as i32 => Some(JoypadButton::SELECT),
        value if value == sys::SDL_KeyCode::SDLK_UP as i32 => Some(JoypadButton::UP),
        value if value == sys::SDL_KeyCode::SDLK_DOWN as i32 => Some(JoypadButton::DOWN),
        value if value == sys::SDL_KeyCode::SDLK_LEFT as i32 => Some(JoypadButton::LEFT),
        value if value == sys::SDL_KeyCode::SDLK_RIGHT as i32 => Some(JoypadButton::RIGHT),
        _ => None,
    };

    if let Some(button) = button {
        cpu.set_button_pressed(button, pressed);
    }
}

fn handle_user_input(cpu: &mut CPU, event_pump: &mut EventPump) {
    let _keep_event_pump_alive = event_pump;

    loop {
        // `SDL_PollEvent` fills in a raw C-style union. `MaybeUninit` lets us allocate space for
        // it safely before SDL writes the actual event bytes.
        let mut raw_event = MaybeUninit::<sys::SDL_Event>::uninit();
        let has_event = unsafe { sys::SDL_PollEvent(raw_event.as_mut_ptr()) == 1 };
        if !has_event {
            break;
        }

        let raw_event = unsafe { raw_event.assume_init() };
        let event_type = unsafe { raw_event.type_ };

        match event_type {
            value if value == sys::SDL_EventType::SDL_QUIT as u32 => process::exit(0),
            value if value == sys::SDL_EventType::SDL_KEYDOWN as u32 => {
                let keycode = unsafe { raw_event.key.keysym.sym };
                if keycode == sys::SDL_KeyCode::SDLK_ESCAPE as i32 {
                    process::exit(0);
                }
                set_button_for_key(cpu, keycode, true);
            }
            value if value == sys::SDL_EventType::SDL_KEYUP as u32 => {
                let keycode = unsafe { raw_event.key.keysym.sym };
                set_button_for_key(cpu, keycode, false);
            }
            _ => {}
        }
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

    // We are only using SDL for keyboard input right now. Rendering still comes later with PPU
    // support.
    let sdl_context = sdl2::init().map_err(|error| format!("Failed to initialize SDL: {error}"))?;
    let video_subsystem = sdl_context
        .video()
        .map_err(|error| format!("Failed to initialize SDL video subsystem: {error}"))?;
    let _window = video_subsystem
        .window("rom-emulator", 256, 240)
        .position_centered()
        .build()
        .map_err(|error| format!("Failed to create SDL window: {error}"))?;
    let mut event_pump = sdl_context
        .event_pump()
        .map_err(|error| format!("Failed to create SDL event pump: {error}"))?;

    let mut cpu = CPU::new(Bus::new(rom));
    cpu.reset();
    println!("Starting execution at: {:#06x}", cpu.program_counter);
    // Run the CPU continuously, but poll SDL events between instructions so ROMs can read live
    // controller state through 0x4016/0x4017.
    cpu.run_with_callback(move |cpu| {
        handle_user_input(cpu, &mut event_pump);
    });

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
