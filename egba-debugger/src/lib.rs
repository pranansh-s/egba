use std::{
    fs::File,
    io::{stdout, BufWriter, Write},
    path::Path,
    sync::Once,
};

use crossterm::{
    cursor::MoveTo,
    terminal::{self, Clear, ClearType},
    ExecutableCommand,
};
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
    fn show_stats(&mut self);
    fn dump_screenshot(&self, path: &Path) -> std::io::Result<()>;
}

static OVERLAY_INIT: Once = Once::new();

impl EGBADebugger for GBA {
    fn show_stats(&mut self) {
        OVERLAY_INIT.call_once(|| {
            let _ = stdout().execute(terminal::SetSize(180, 40));
        });
        let _ = stdout().execute(Clear(ClearType::All));
        let _ = stdout().execute(MoveTo(0, 0));

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
