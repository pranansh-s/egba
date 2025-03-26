use std::{io::{stdout, Write}, process::Command};

use crossterm::{terminal, ExecutableCommand};
use egba_core::{cpu::psr::OperatingState, gba::GBA};
use ratatui::{layout::{Constraint, Direction, Layout}, prelude::CrosstermBackend, widgets::{Block, Borders, List, ListItem, Paragraph}, Terminal};

mod decoder;
use decoder::{arm::arm_decode, thumb::thumb_decode};

pub trait EGBADebugger {
    fn show_stats(&mut self) {}
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

        terminal.draw(|f| {
            let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Fill(0), Constraint::Percentage(66)].as_ref())
            .split(f.area());

            let left_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(66),
                    Constraint::Percentage(34),
                ])
                .split(chunks[0]);

            let registers: Vec<ListItem> = self
                .cpu
                .reg
                .iter()
                .enumerate()
                .map(|(i, r)| ListItem::new(format!("R{:02}: {:#010x} ({r})", i, r)))
                .collect();

            let reg_list = List::new(registers)
                .block(Block::default().borders(Borders::ALL));

            let cpsr = self.cpu.cpsr;
            let cpsr_text = format!(
                "Mode: {:?} | State: {:?}\nFIQ: {} | IRQ: {}\nN: {} | Z: {} | C: {} | V: {}",
                cpsr.mode, cpsr.operating_state, 
                cpsr.fiq_disable_bit, cpsr.irq_disable_bit,
                cpsr.n_condition_bit, cpsr.z_condition_bit, cpsr.c_condition_bit, cpsr.v_condition_bit
            );

            let cpsr_widget = Paragraph::new(cpsr_text)
                .block(Block::default().title("Current Program Status Register").borders(Borders::ALL));

            let pc_value = match cpsr.operating_state {
                OperatingState::ARM => self.cpu.arm_pc(),
                OperatingState::THUMB => self.cpu.thumb_pc()
            };
            
            let instruction = self.cpu.pipeline[1];
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
        }).unwrap();

        terminal.backend_mut().flush().ok();
    }
}