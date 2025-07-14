## EGBA - GBA Emulator in rust
EGBA is a modular Game Boy Advance emulator suite written in Rust, featuring:
- Cycle-accurate ARM7TDMI CPU emulation
- Hardware-perfect component simulation
- Dual-interface operation (SDL2 GUI and TUI debugger)
- Extensible library-based architecture

## Features
**CPU**
  - Cycle-accurate ARMv4T architecture
  - Full ARM/THUMB instruction sets
  - Barrel shifter and pipeline implementation
  - Exception handling (SWI, IRQ, FIQ)

**Memory Subsystem**
  - Memory-mapped I/O registers
  - Wait-state controlled access

**Hardware**
  - PPU with modes 0-5 rendering
  - APU with 4-channel sound
  - Controllable keypad state
  - Cartridge backup support

**Other**
  - Robust debugging with a UI from managed states
  - Managing output of graphics/audio using sdl2
  
```CURRENTLY DEVELOPING```