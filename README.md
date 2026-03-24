## EGBA - GBA Emulator in Rust
EGBA is a modular Game Boy Advance emulator suite written in Rust with a library based architecture

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
  - Output of GUI and sound using sdl2
  
```CURRENTLY DEVELOPING```
