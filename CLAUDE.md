# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run

```bash
cargo build              # Build all workspace members
cargo run -- -b <bios> -r <rom> [-s <backup>] [-d]  # Run emulator
cargo test               # Run tests
```

The emulator requires a GBA BIOS file and ROM file. Use `-d` flag to enable debug mode with register/instruction display.

## Architecture

**Workspace structure** (Cargo workspace with `resolver = "2"`):

- `egba-core/` — Emulation library (CPU, memory, hardware subsystems)
- `egba-ui/` — SDL2-based video output and input handling
- `egba-debugger/` — Terminal-based debugger using ratatui
- `emulator/` — Binary crate that integrates all components

**Core emulation loop** (`egba-core/src/gba.rs`):
- `GBA::step()` — Single CPU cycle + hardware updates (video, timers, DMA, interrupts)
- `GBA::run_frame()` — Executes 280,896 cycles per frame (60 FPS target)

**Key subsystems**:
- **CPU** (`cpu/`): ARMv4T + Thumb instruction sets, pipeline emulation, exception handling
- **Video** (`video/`): PPU with background/sprite rendering, scanline-based timing
- **Memory** (`memory.rs`): Memory-mapped I/O with wait-state simulation
- **Cartridge** (`cartridge/`): ROM loading + backup media (SRAM/EEPROM/Flash)

**Bus interface**: All hardware components implement `Bus` trait for byte-level read/write operations at memory-mapped addresses.

**Debug mode**: When enabled, `GBA` implements `EGBADebugger` trait showing registers, CPSR flags, and decoded instructions via ratatui TUI.
