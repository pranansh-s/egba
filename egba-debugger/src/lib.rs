use std::{
    fs::File,
    io::{stdout, BufWriter, Write},
    path::Path,
    process::Command,
};

use crossterm::{terminal, ExecutableCommand};
use egba_core::{
    cpu::psr::OperatingState,
    gba::{FB_HEIGHT, FB_WIDTH, GBA},
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::CrosstermBackend,
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};

mod decoder;
use decoder::{arm::arm_decode, thumb::thumb_decode};

pub trait EGBADebugger {
    fn show_stats(&mut self) {}
    fn dump_trace(&mut self, n: u32, log_path: &Path) -> std::io::Result<()>;
    fn dump_trace_until(
        &mut self,
        max_n: u32,
        break_pc: Option<u32>,
        watch: &[u32],
        log_path: &Path,
    ) -> std::io::Result<()>;
    fn dump_screenshot(&self, path: &Path) -> std::io::Result<()>;
    fn dump_io_snapshot(&self) -> String;
}

impl EGBADebugger for GBA {
    fn show_stats(&mut self) {
        Command::new("clear").status().ok();
        stdout().execute(terminal::SetSize(180, 40)).unwrap();

        let mut terminal = match Terminal::new(CrosstermBackend::new(stdout())) {
            Ok(t) => t,
            Err(err) => {
                eprintln!("DB/: Failed to create terminal due to err: {err}");
                return;
            }
        };

        terminal
            .draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Fill(0), Constraint::Percentage(66)].as_ref())
                    .split(f.area());

                let left_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(66), Constraint::Percentage(34)])
                    .split(chunks[0]);

                let cpu = self.get_cpu();
                let registers: Vec<ListItem> = cpu
                    .reg
                    .iter()
                    .enumerate()
                    .map(|(i, r)| ListItem::new(format!("R{:02}: {:#010x} ({r})", i, r)))
                    .collect();

                let reg_list = List::new(registers).block(Block::default().borders(Borders::ALL));

                let cpsr = cpu.cpsr;
                let cpsr_text = format!(
                    "Mode: {:?} | State: {:?}\nFIQ: {} | IRQ: {}\nN: {} | Z: {} | C: {} | V: {}",
                    cpsr.mode,
                    cpsr.operating_state,
                    cpsr.fiq_disable_bit,
                    cpsr.irq_disable_bit,
                    cpsr.n_condition_bit,
                    cpsr.z_condition_bit,
                    cpsr.c_condition_bit,
                    cpsr.v_condition_bit
                );

                let cpsr_widget = Paragraph::new(cpsr_text).block(
                    Block::default()
                        .title("Current Program Status Register")
                        .borders(Borders::ALL),
                );

                let pc_value = match cpsr.operating_state {
                    OperatingState::ARM => cpu.arm_pc(),
                    OperatingState::THUMB => cpu.thumb_pc(),
                };

                let instruction = cpu.pipeline[1];
                let decoded_instruction = match cpsr.operating_state {
                    OperatingState::ARM => arm_decode(instruction),
                    OperatingState::THUMB => thumb_decode(instruction),
                };

                let instruction_text = Paragraph::new(format!(
                    "PC: {:#010x} ({pc_value})  |  Current Instruction: {:#032b}\n\nDecoded Instruction: {decoded_instruction}",
                    pc_value, instruction
                ))
                .block(Block::default().borders(Borders::ALL));

                f.render_widget(reg_list, left_chunks[0]);
                f.render_widget(instruction_text, chunks[1]);
                f.render_widget(cpsr_widget, left_chunks[1]);
            })
            .unwrap();

        terminal.backend_mut().flush().ok();
    }

    fn dump_trace(&mut self, n: u32, log_path: &Path) -> std::io::Result<()> {
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = File::create(log_path)?;
        let mut w = BufWriter::new(file);

        for i in 0..n {
            if i % 10_000 == 0 {
                writeln!(w, "; {}", self.dump_io_snapshot())?;
            }
            let cpu = self.get_cpu();
            let cpsr = cpu.cpsr;
            let state_char = match cpsr.operating_state {
                OperatingState::ARM => 'A',
                OperatingState::THUMB => 'T',
            };
            let pc = match cpsr.operating_state {
                OperatingState::ARM => cpu.arm_pc(),
                OperatingState::THUMB => cpu.thumb_pc(),
            };
            let instr = cpu.pipeline[1];
            let cpsr_word: u32 = cpsr.into();
            let r0 = cpu.reg[0];
            let r1 = cpu.reg[1];
            let r2 = cpu.reg[2];
            let r3 = cpu.reg[3];
            let r12 = cpu.reg[12];
            let sp = cpu.reg[13];
            let lr = cpu.reg[14];
            let cyc = self.bus_cycles();

            writeln!(
                w,
                "{:08} cyc={:10} {} pc={:08X} instr={:08X} cpsr={:08X} r0={:08X} r1={:08X} r2={:08X} r3={:08X} r12={:08X} sp={:08X} lr={:08X}",
                i, cyc, state_char, pc, instr, cpsr_word, r0, r1, r2, r3, r12, sp, lr,
            )?;

            self.step_one_instruction();
        }

        w.flush()?;
        Ok(())
    }

    fn dump_trace_until(
        &mut self,
        max_n: u32,
        break_pc: Option<u32>,
        watch: &[u32],
        log_path: &Path,
    ) -> std::io::Result<()> {
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = File::create(log_path)?;
        let mut w = BufWriter::new(file);

        let mut watched: Vec<(u32, u32)> = watch.iter().map(|&a| (a, self.read_word(a))).collect();

        writeln!(w, "; {}", self.dump_io_snapshot())?;
        for (a, v) in &watched {
            writeln!(w, "; watch[{:08X}] = {:08X}", a, v)?;
        }

        for i in 0..max_n {
            let cpu = self.get_cpu();
            let cpsr = cpu.cpsr;
            let pc = match cpsr.operating_state {
                OperatingState::ARM => cpu.arm_pc(),
                OperatingState::THUMB => cpu.thumb_pc(),
            };

            if let Some(bp) = break_pc {
                if pc == bp {
                    writeln!(w, "; BREAK at PC={:08X} after {} instructions", pc, i)?;
                    writeln!(w, "; {}", self.dump_io_snapshot())?;
                    let cpu = self.get_cpu();
                    for (idx, r) in cpu.reg.iter().enumerate() {
                        writeln!(w, ";   R{:02} = {:08X}", idx, r)?;
                    }
                    break;
                }
            }

            let state_char = match cpsr.operating_state {
                OperatingState::ARM => 'A',
                OperatingState::THUMB => 'T',
            };
            let instr = cpu.pipeline[1];
            let cpsr_word: u32 = cpsr.into();
            let cyc = self.bus_cycles();
            let regs = cpu.reg;
            writeln!(
                w,
                "{:08} cyc={:10} {} pc={:08X} instr={:08X} cpsr={:08X} r0={:08X} r1={:08X} r2={:08X} r3={:08X} r12={:08X} sp={:08X} lr={:08X}",
                i, cyc, state_char, pc, instr, cpsr_word,
                regs[0], regs[1], regs[2], regs[3], regs[12], regs[13], regs[14],
            )?;

            self.step_one_instruction();

            for (addr, prev) in watched.iter_mut() {
                let now = self.read_word(*addr);
                if now != *prev {
                    let pc_after = match self.get_cpu().cpsr.operating_state {
                        OperatingState::ARM => self.get_cpu().arm_pc(),
                        OperatingState::THUMB => self.get_cpu().thumb_pc(),
                    };
                    writeln!(
                        w,
                        "; WATCH [{:08X}] {:08X} -> {:08X} (after instr #{} at PC={:08X})",
                        addr, prev, now, i, pc_after
                    )?;
                    *prev = now;
                }
            }
        }

        w.flush()?;
        Ok(())
    }

    fn dump_io_snapshot(&self) -> String {
        let dispcnt = self.read_hword(0x0400_0000);
        let dispstat = self.read_hword(0x0400_0004);
        let vcount = self.read_hword(0x0400_0006);
        let ie = self.read_hword(0x0400_0200);
        let if_ = self.read_hword(0x0400_0202);
        let ime = self.read_hword(0x0400_0208);
        let waitcnt = self.read_hword(0x0400_0204);
        let irq_vec = self.read_word(0x0300_7FFC);
        let irq_vec_mirror = self.read_word(0x03FF_FFFC);
        format!(
            "io: DISPCNT={:04X} DISPSTAT={:04X} VCOUNT={:04X} IE={:04X} IF={:04X} IME={:04X} WAITCNT={:04X} VEC[3007FFC]={:08X} VEC[3FFFFFC]={:08X} cyc={}",
            dispcnt, dispstat, vcount, ie, if_, ime, waitcnt, irq_vec, irq_vec_mirror, self.bus_cycles()
        )
    }

    fn dump_screenshot(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let fb = self.framebuffer();
        let file = File::create(path)?;
        let mut w = BufWriter::new(file);
        writeln!(w, "P6")?;
        writeln!(w, "{} {}", FB_WIDTH, FB_HEIGHT)?;
        writeln!(w, "255")?;
        for px in fb.iter() {
            let r = ((px >> 16) & 0xFF) as u8;
            let g = ((px >> 8) & 0xFF) as u8;
            let b = (px & 0xFF) as u8;
            w.write_all(&[r, g, b])?;
        }
        w.flush()?;
        Ok(())
    }

}
