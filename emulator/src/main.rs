use std::{
    fs,
    path::{Path, PathBuf},
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
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    println!("Escape key pressed. Exiting.");
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

        let audio_samples = gba.drain_audio();
        ui.queue_audio(&audio_samples);

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
            Arg::new("trace")
                .help("Trace N instructions to docs/captures/trace.log and exit")
                .long("trace")
                .value_parser(clap::value_parser!(u32)),
        )
        .arg(
            Arg::new("trace-out")
                .help("Trace log path (default docs/captures/trace.log)")
                .long("trace-out")
                .value_parser(clap::value_parser!(PathBuf)),
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
        .arg(
            Arg::new("break-pc")
                .help("Stop trace when PC reaches this hex address (e.g. 080000C0)")
                .long("break-pc"),
        )
        .arg(
            Arg::new("watch")
                .help("Comma-separated hex addresses to log on word-value change (e.g. 03007FFC,04000004)")
                .long("watch"),
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

    if let Some(&n) = args.get_one::<u32>("trace") {
        let default_path = PathBuf::from("docs/captures/trace.log");
        let out: &Path = args
            .get_one::<PathBuf>("trace-out")
            .map(PathBuf::as_path)
            .unwrap_or(&default_path);

        let break_pc = args
            .get_one::<String>("break-pc")
            .map(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).expect("invalid --break-pc hex"));
        let watch: Vec<u32> = args
            .get_one::<String>("watch")
            .map(|s| {
                s.split(',')
                    .map(|t| u32::from_str_radix(t.trim().trim_start_matches("0x"), 16).expect("invalid --watch hex"))
                    .collect()
            })
            .unwrap_or_default();

        let result = if break_pc.is_some() || !watch.is_empty() {
            egba.dump_trace_until(n, break_pc, &watch, out)
        } else {
            egba.dump_trace(n, out)
        };
        result.unwrap_or_else(|err| {
            eprintln!("Error: {}", err);
            std::process::exit(1);
        });
        return;
    }

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
        return;
    }

    let mut egba_ui = EgbaUI::new().unwrap_or_else(|err| {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    });

    run(&mut egba_ui, &mut egba, debug);
}
