# External ROM Support Plan

## Current State

- `main.rs` does not load ROMs. It embeds a hardcoded Snake byte program and uses a flat CPU memory array.
- `cartridge.rs` already parses iNES ROM headers and extracts `prg_rom`, `chr_rom`, mapper, and mirroring.
- `bus.rs` already maps CPU RAM and PRG ROM into the CPU address space.
- `bus.rs` does not implement PPU registers yet; reads currently return `0`.
- There is no real PPU implementation in the repo.
- There is no controller or joypad device abstraction yet; current input writes directly to RAM addresses for the custom Snake program.
- `cargo test` is currently failing in `cartridge.rs`, so the test baseline needs cleanup before relying on tests.

## What "Run External ROMs" Means

There are two different targets:

1. Minimal target: load simple `.nes` files and execute CPU code.
   This can work for CPU-only test ROMs or ROMs that do not depend on rendering, audio, or standard NES input.
2. Real target: run actual NES games.
   This requires CPU + bus + PPU + controller input + timing, and eventually mapper support beyond the simplest case.

## Recommended Scope

Start with NROM only:

- mapper `0`
- iNES 1.0 only
- PRG ROM loading
- basic CHR ROM support
- one controller
- no APU initially

That gives a path to run simple early NES ROMs and test ROMs without committing to full emulator complexity up front.

## Plan

### 1. Unify the runtime around `Bus` and `Rom`

- Refactor `CPU` so it uses `Bus` for all memory access instead of its internal `[u8; 0xFFFF]`.
- Remove the split architecture where `main.rs` uses direct CPU memory but `trace.rs` and `bus.rs` imply a bus-backed CPU.
- Outcome: all program loading and device access goes through one memory model.

#### Detailed Step 1 Plan

Goal: replace the current split memory architecture with a single bus-backed runtime so every CPU memory access goes through `Bus`, and both the emulator runtime and tests use the same model.

Why this step exists:

- `main.rs` currently runs a custom path: `CPU::new()`, `cpu.load(game_code)`, `cpu.reset()`.
- That path depends on `CPU` owning raw memory and loading code directly into RAM.
- `bus.rs` and `cartridge.rs` already represent the architecture needed for ROM loading, but they are not used by the live runtime.
- `trace.rs` tests already assume a future bus-backed constructor (`CPU::new(bus)`), which means the repo is partway through an architectural transition.
- External ROM support will stay awkward until there is exactly one memory path.

Success criteria:

- `CPU` no longer owns the main memory array.
- `CPU` delegates all memory access to `Bus`.
- `main.rs`, `trace.rs`, and tests all construct `CPU` the same way.
- Temporary test and program-loading helpers still exist for non-ROM unit tests.
- `cargo build` passes after the refactor.
- The codebase is ready for Step 2 without another architecture change.

Detailed work breakdown:

1. Map every place that currently assumes CPU-owned memory.
   - Inspect all code paths that access memory through `CPU`.
   - Confirm whether any code writes directly to `self.memory[...]` besides `CPU::load()`.
   - Expected hotspots:
     - `CPU::new`
     - `CPU::load`
     - `CPU::reset`
     - `Mem for CPU`
     - stack helpers
     - trace and test setup
   - Purpose: avoid missing hidden direct-memory assumptions during the refactor.

2. Define the target `CPU` shape.
   - Current `CPU` contains:
     - registers
     - status flags
     - stack pointer
     - program counter
     - raw memory array
   - Target `CPU` should contain:
     - registers
     - status flags
     - stack pointer
     - program counter
     - `bus: Bus`
   - Keep instruction execution inside `CPU`.
   - Keep address-space routing and storage ownership inside `Bus`.

3. Refactor `CPU::new`.
   - Change constructor signature from `CPU::new()` to `CPU::new(bus: Bus)`.
   - Initialize registers exactly as today.
   - Remove memory-array initialization from the constructor.
   - Verify every callsite that creates a CPU:
     - `main.rs`
     - `trace.rs` tests
     - any CPU tests in `cpu.rs`

4. Remove `memory: [u8; 0xFFFF]` from `CPU`.
   - Delete the field entirely.
   - This is the core architectural change that forces all memory access through one layer.
   - Double-check that no instruction helpers rely on indexing raw memory directly.

5. Change `Mem for CPU` into pure delegation.
   - Keep `impl Mem for CPU`.
   - Update:
     - `mem_read` to call `self.bus.mem_read(addr)`
     - `mem_write` to call `self.bus.mem_write(addr, data)`
   - This preserves most of the instruction implementation unchanged, because the CPU already uses `mem_read` and `mem_write` helpers in many places.

6. Audit helper methods for accidental direct-memory dependencies.
   - Verify these still work unchanged once `Mem` delegates into `Bus`:
     - `mem_read_u16`
     - `mem_write_u16`
     - `get_operand_address`
     - stack helpers
     - instruction implementations like `lda`, `sta`, `adc`, `jmp`, and related methods
   - Special attention:
     - zero-page wraparound behavior
     - stack page `0x0100`
     - reset vector reads at `0xFFFC`
   - Goal: ensure the architectural change does not alter CPU semantics.

7. Decide what to do with `CPU::load`.

   There are two reasonable options:

   1. Keep `CPU::load`, but redefine it as a test and helper path.
      - Use it only for injected programs and temporary Snake runtime support.
      - It should write through `mem_write` rather than touching a raw memory array.
      - This minimizes test churn short term.

   2. Remove `CPU::load` from production intent and replace it with explicit setup helpers.
      - Example: tests manually write bytes through bus and CPU memory helpers.
      - Cleaner long term, but more callsite churn now.

   Recommended: keep `CPU::load` temporarily as a helper, but treat it as non-cartridge setup only.

8. Reassess the `CPU::load` load address.
   - Right now `CPU::load()` writes to `0x0600` and writes the reset vector to `0xFFFC`.
   - That is fine for injected test programs and the current Snake program, but it is not ROM behavior.
   - For Step 1, it can stay as a helper if:
     - it writes via `mem_write`
     - it is clearly understood as a test and development bootstrap path
   - Do not try to turn it into ROM loading yet.

9. Check whether `Bus` can support the temporary helper path.
   - `Bus` currently maps:
     - RAM: `0x0000..=0x1FFF`
     - PPU registers: `0x2000..=0x3FFF` stubbed
     - PRG ROM: `0x8000..=0xFFFF`
   - `CPU::load()` currently uses `0x0600`, which falls inside RAM and should still work through `Bus`.
   - Writing the reset vector to `0xFFFC` will not work with the current `Bus`, because writes to ROM space panic.
   - This is the biggest concrete issue for Step 1.

10. Solve the reset-vector conflict for injected programs.

    There are two viable approaches:

    1. Make `CPU::load()` stop writing the reset vector and instead set `program_counter` directly for helper-program flows.
       - Example behavior:
         - write bytes to RAM at `0x0600`
         - `reset()` no longer depends on a ROM-space write for this helper flow
         - or provide a separate helper to set `program_counter = 0x0600`
       - This avoids violating ROM mapping.

    2. Extend `Bus` with a small reset-vector override for test and helper mode.
       - More machinery than needed for this step.

    Recommended: for Step 1, stop treating `CPU::load()` as a cartridge-like reset-vector writer.

11. Refactor `CPU::reset` expectations carefully.
   - Today `reset()` reads `0xFFFC`.
   - That is correct for real cartridge-backed execution.
   - Keep `reset()` doing that, because it matches the long-term design.
   - For test and helper flows that inject code into RAM instead of ROM:
     - either avoid `reset()`
     - or provide setup helpers/tests that set `program_counter` explicitly after register reset
   - This keeps the emulator semantics correct while allowing temporary non-ROM program injection.

12. Update `main.rs` to use a bus-backed CPU.
   - `main.rs` should stop constructing a bare CPU.
   - It should create:
     - a temporary `Rom` or bus-compatible bootstrap setup
     - a `Bus`
     - `CPU::new(bus)`
   - Because Step 2 is not implemented yet, `main.rs` will likely still use the embedded Snake code temporarily.
   - There are two practical ways to do that:
     1. keep using helper RAM injection for the Snake program
     2. create a synthetic ROM and map Snake through `Bus`
   - Recommended for Step 1: keep Snake on helper RAM injection to minimize scope.

13. Align `trace.rs` with the real constructor.
   - `trace.rs` tests already use `CPU::new(bus)`, which is good.
   - After the constructor refactor, those tests become architecturally correct.
   - Check whether those tests rely on writing into ROM space or only RAM.
   - If they rely on helper loading, align them with the same helper model used elsewhere.

14. Review `cpu.rs` unit tests for hidden assumptions.
   - Any tests in `cpu.rs` that assume:
     - direct writable full address space
     - `CPU::new()`
     - `load_and_run()` semantics tied to `0x0600`
   - These will need targeted updates.
   - The goal is not to redesign all tests, only to make them compatible with bus-backed memory.

15. Stabilize `Bus` enough for Step 1.
   - No feature expansion needed yet.
   - But verify current behavior is sufficient for CPU-backed execution:
     - RAM writes and readbacks work
     - mirrored RAM behavior remains intact
     - PRG ROM reads work
     - ROM writes still panic
   - If needed, add the minimum helper and test support without introducing PPU or controller logic.

16. Decide how to represent "no cartridge yet" during Step 1.

   Options:

   1. Build `Bus` only with a `Rom`.
      - Then helper-program execution needs a synthetic ROM or alternative setup.

   2. Allow a simple bootstrap and test ROM path.
      - Use the existing `cartridge::test::test_rom` pattern or a dedicated dummy ROM builder.

   Recommended: use a synthetic ROM for constructor stability where needed.

17. Resolve the mismatch between helper RAM execution and cartridge ROM execution.
   - Step 1 does not need to eliminate helper execution.
   - It only needs to ensure helper execution also runs through `Bus`.
   - That means:
     - helper programs can still live in RAM
     - real cartridges will later live in PRG ROM
     - both paths share one memory interface

18. Verification plan for Step 1.
   - Run these in order after implementation:
     1. `cargo build`
        - Confirms constructor and callsite consistency and compile integrity.
     2. narrow runtime smoke test
        - Confirm the current Snake path still starts with the new bus-backed runtime.
        - No ROM loading yet, just validate that the refactor did not break the current executable path.
     3. `cargo test`
        - Expect existing failures in `cartridge.rs` unless fixed separately.
        - Separate pre-existing failures from refactor regressions.
        - Goal: no new architectural failures from the CPU and bus change.

19. Definition of done.
   - `CPU` stores `Bus`, not raw memory.
   - `Mem for CPU` is delegation only.
   - The embedded Snake runtime uses a bus-backed CPU.
   - `trace.rs` constructor usage matches production code.
   - No remaining active runtime path bypasses `Bus`.
   - Remaining failures, if any, are pre-existing and documented rather than caused by the refactor.

Files likely involved:

- `cpu.rs`
  - constructor
  - memory field removal
  - `Mem` delegation
  - helper loading and reset semantics
- `main.rs`
  - CPU construction
  - temporary Snake bootstrap path
- `trace.rs`
  - constructor alignment
  - possible helper setup changes
- `bus.rs`
  - likely minimal changes only, unless test and bootstrap support is needed
- possibly `cartridge.rs`
  - only if a synthetic ROM helper is reused or cleaned up for bootstrap or tests

Main technical decisions to make before coding:

1. Should `CPU::load()` remain temporarily as a helper API?
   - Recommended: yes, but only for RAM-injected test and development programs.
2. Should helper program startup use `reset()`?
   - Recommended: no, not if it still depends on writing a reset vector into ROM space.
   - Prefer explicit `program_counter` setup for helper flows until real cartridge boot is implemented.
3. Should Step 1 also fix the broken `cartridge.rs` tests?
   - Recommended: only if needed to keep the baseline understandable.
   - Otherwise keep Step 1 tightly scoped to memory unification.

Recommended implementation order:

1. Change `CPU` to own `Bus`.
2. Delegate `Mem` through `Bus`.
3. Remove the raw CPU memory field.
4. Update helper load behavior to avoid ROM-space reset-vector writes.
5. Update `main.rs` construction and runtime bootstrap.
6. Update `trace.rs` and CPU tests.
7. Build and smoke test.
8. Run tests and classify failures.

Biggest risk:

- The biggest practical risk is not the constructor change.
- It is the current helper-program flow depending on writable reset-vector space, which stops making sense once ROM becomes read-only through `Bus`.
- That is the seam to handle carefully in Step 1.

### 2. Replace hardcoded Snake boot with file-based ROM loading

- Accept a ROM path from the command line.
- Read the `.nes` file from disk.
- Parse it through `Rom::new`.
- Reject unsupported formats clearly:
  - NES 2.0
  - unsupported mapper
- Outcome: `cargo run -- path/to/game.nes`.

### 3. Support mapper 0 cleanly

- Keep the existing PRG ROM mapping logic in `bus.rs`.
- Verify 16 KB PRG mirroring and 32 KB PRG cases.
- Keep mapper handling explicit:
  - support only mapper `0`
  - error for everything else
- Outcome: valid NROM games can at least execute CPU code against the right ROM layout.

### 4. Implement reset and startup from cartridge vectors

- Ensure the reset vector comes from PRG ROM via the bus at `0xFFFC`.
- Remove the current custom `load()` behavior that writes code to `0x0600`.
- Outcome: the CPU starts like an NES, from the ROM's reset vector.

### 5. Add controller input as an NES device

- Replace the current custom "write `WASD` into RAM" model.
- Implement controller strobing and register reads through the proper CPU-visible addresses, typically `0x4016` and `0x4017`.
- Map keyboard input to NES buttons:
  - arrows for D-pad
  - `Z` and `X` or similar for `A` and `B`
  - `Enter` and `Right Shift` for `Start` and `Select`
- Outcome: external ROMs can read input the way they expect.

### 6. Implement a minimal PPU

This is the biggest missing piece.

- Add PPU state and connect it to bus-mapped registers.
- Implement CPU-visible PPU registers:
  - `PPUCTRL`
  - `PPUMASK`
  - `PPUSTATUS`
  - `OAMADDR`
  - `OAMDATA`
  - `PPUSCROLL`
  - `PPUADDR`
  - `PPUDATA`
- Add VRAM, palette RAM, OAM, and nametable mirroring handling.
- Use `chr_rom` as pattern table data for mapper 0.
- Outcome: games can render instead of writing into the custom 32x32 RAM framebuffer.

### 7. Replace the custom framebuffer renderer with NES frame rendering

- Stop reading screen pixels from `0x0200..0x0600`.
- Render from PPU output to SDL.
- Use a real NES frame size of `256x240`.
- Scale that up in SDL as the current app already does.
- Outcome: actual NES graphics appear on screen.

### 8. Add timing between CPU and PPU

- NES timing matters.
- The CPU and PPU should advance in lockstep with the correct ratio.
- Even if timing is not perfect immediately, the emulator needs a coherent stepping model.
- Outcome: games become stable enough to boot and draw correctly.

### 9. Add NMI and vblank behavior

- Many games rely on vblank NMIs from the PPU.
- CPU interrupt handling must work correctly with the PPU's timing.
- Outcome: menus and gameplay logic that depend on vblank start working.

### 10. Add a test ROM workflow

- Add support for known emulator test ROMs.
- Start with CPU-focused ROMs, then PPU-focused ROMs.
- Use these before trying arbitrary commercial ROMs.
- Outcome: progress becomes measurable instead of guesswork.

### 11. Fix the current test baseline

- `cargo test` already fails in `cartridge.rs`.
- Fix those broken assertions first so there is a stable starting point.
- Then add tests for:
  - ROM parsing
  - mapper 0 PRG mapping
  - reset vector behavior
  - controller register behavior
  - PPU register mirroring and data access
- Outcome: safer iteration as the emulator grows.

### 12. Optional later phases

- APU and audio
- Save RAM
- More mappers like MMC1, UxROM, and CNROM
- Better ROM compatibility
- Debugger and trace mode

## Suggested Milestones

### 1. CPU-only external ROM loading

- Load `.nes`
- parse iNES
- support mapper 0
- boot from reset vector
- no graphics expectation yet

### 2. Basic visible boot

- minimal PPU register support
- render `256x240` frame
- show title screens or simple output

### 3. Playable mapper-0 games

- controller input
- timing
- NMI and vblank
- stable frame loop

### 4. Broader compatibility

- more accurate PPU behavior
- more mappers
- audio

## Risks and Gaps

- The codebase already shows two competing architectures:
  - direct CPU memory in `main.rs`
  - bus and cartridge model in `bus.rs`
- That needs consolidation before feature work.
- PPU is effectively absent, which is the main reason external ROMs are not just a small wiring task.
- Some tests and code paths look unfinished or stale, so there will be some cleanup before clean forward progress.

## Effort Estimate

- External ROM loading only, CPU-side only: small
- External ROMs with actual visible NES game output: medium to large
- External ROMs with good game compatibility: large

## Recommended First Implementation Slice

1. Fix test baseline.
2. Refactor CPU to use `Bus`.
3. Load `.nes` from CLI.
4. Support mapper 0 only.
5. Boot from reset vector.
6. Add controller abstraction.
7. Then begin PPU work.
