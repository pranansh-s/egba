use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use clap::{command, Arg};
use egba_core::{bios::Bios, cartridge::Cartridge, gba::GBA, rom::Rom};
use egba_debugger::EGBADebugger;
use egba_ui::{
    window::{get_keystate, EgbaUI},
    Event, Keycode,
};

const FRAME_DURATION: Duration = Duration::from_nanos(1_000_000_000 / 60);

fn run(ui: &mut EgbaUI, gba: &mut GBA, debug: bool) {
    let mut event_pump = ui
        .get_event_pump()
        .expect("Failed to create SDL2 event pump");

    '_game: loop {
        let frame_start = Instant::now();

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    println!("Quit event received. Exiting.");
                    return;
                }
                Event::KeyUp {
                    keycode: Some(keycode),
                    ..
                } => match keycode {
                    Keycode::Escape => {
                        println!("Escape key pressed. Exiting.");
                        return;
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        let keystate = get_keystate(&event_pump);
        gba.update_keypad(keystate);

        if debug {
            gba.show_stats();
        }
        gba.run_frame();

        ui.render_frame(gba.framebuffer());

        let elapsed = frame_start.elapsed();
        if elapsed < FRAME_DURATION {
            std::thread::sleep(FRAME_DURATION - elapsed);
        }
    }
}

fn main() {
    let args = command!()
        .arg(
            Arg::new("bios")
                .help("Enter BIOS file path")
                .short('b')
                .long("bios")
                .value_parser(clap::value_parser!(PathBuf))
                .required(true),
        )
        .arg(
            Arg::new("rom")
                .help("Enter ROM file path")
                .short('r')
                .long("rom")
                .value_parser(clap::value_parser!(PathBuf))
                .required(true),
        )
        .arg(
            Arg::new("backup")
                .help("Enter Backup file path")
                .short('s')
                .long("backup")
                .value_parser(clap::value_parser!(PathBuf))
                .required(false),
        )
        .arg(
            Arg::new("debug")
                .help("Enable debug mode")
                .short('d')
                .long("debug")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let bios_path = args
        .get_one::<PathBuf>("bios")
        .expect("Failed to read BIOS ROM path");
    let bios_buffer = fs::read(bios_path).unwrap();
    let bios_rom = Rom::new(&bios_buffer);
    let bios = Bios::new(bios_rom).unwrap_or_else(|err| {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    });

    let rom_path = args
        .get_one::<PathBuf>("rom")
        .expect("Failed to read Game ROM path");
    let rom_buffer = fs::read(rom_path).unwrap();
    let rom = Rom::new(&rom_buffer);

    let backup_path = args.get_one::<PathBuf>("backup").unwrap_or(rom_path);
    let mut sav_path = backup_path.to_owned();
    sav_path.set_extension("sav");
    let cartridge = Cartridge::new(rom, &sav_path).unwrap_or_else(|err| {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    });

    let debug = args.get_flag("debug");
    let mut egba = GBA::new(bios, cartridge);
    let mut egba_ui = EgbaUI::new().unwrap_or_else(|err| {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    });

    run(&mut egba_ui, &mut egba, debug);
}
