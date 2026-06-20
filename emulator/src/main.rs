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

    let mut next_frame_at = Instant::now() + FRAME_DURATION;

    '_game: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    println!("Quit event received. Exiting.");
                    gba.save_backup();
                    return;
                }
                Event::KeyUp {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    println!("Escape key pressed. Exiting.");
                    gba.save_backup();
                    return;
                }
                _ => {}
            }
        }
        let keystate = get_keystate(&event_pump);
        gba.update_keypad(keystate);

        if debug {
            gba.show_stats();
            std::thread::sleep(Duration::from_millis(300));
        }

        gba.run_frame();
        ui.render_frame(gba.framebuffer());
        ui.queue_audio(gba.audio_samples());
        gba.clear_audio();

        let now = Instant::now();
        if now < next_frame_at {
            std::thread::sleep(next_frame_at - now);
            next_frame_at += FRAME_DURATION;
        } else {
            let behind = now - next_frame_at;
            if behind > FRAME_DURATION * 4 {
                next_frame_at = now + FRAME_DURATION;
            } else {
                next_frame_at += FRAME_DURATION;
            }
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
        .arg(
            Arg::new("headless")
                .help("Run without SDL window; advance --frames and exit")
                .long("headless")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("frames")
                .help("Frames to advance in headless mode")
                .long("frames")
                .value_parser(clap::value_parser!(u32))
                .default_value("1"),
        )
        .arg(
            Arg::new("screenshot")
                .help("After headless run, dump framebuffer PPM to this path")
                .long("screenshot")
                .value_parser(clap::value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("skip-bios")
                .help("Skip BIOS boot animation; jump straight to cart entry at 0x08000000 with post-BIOS register/SP state")
                .long("skip-bios")
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
    let headless = args.get_flag("headless");
    let skip_bios = args.get_flag("skip-bios");
    let mut egba = if skip_bios {
        GBA::new_skipping_bios(bios, cartridge)
    } else {
        GBA::new(bios, cartridge)
    };

    if headless {
        let frames = *args.get_one::<u32>("frames").unwrap_or(&1);
        for _ in 0..frames {
            egba.run_frame();
        }
        if let Some(path) = args.get_one::<PathBuf>("screenshot") {
            egba.dump_screenshot(path).unwrap_or_else(|err| {
                eprintln!("Error: {}", err);
                std::process::exit(1);
            });
        }
        egba.save_backup();
        return;
    }

    let mut egba_ui = EgbaUI::new().unwrap_or_else(|err| {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    });

    run(&mut egba_ui, &mut egba, debug);
}
