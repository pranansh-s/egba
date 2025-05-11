use std::{fs, path::PathBuf, thread::sleep, time::Duration};

use clap::{command, Arg};
use egba_core::{bios::Bios, cartridge::Cartridge, gba::GBA, keypad::Keypad, rom::Rom};
use egba_debugger::EGBADebugger;
use egba_ui::{window::EgbaUI, Event, Keycode};

const FRAME: Duration = Duration::from_nanos(1_000_000_000 / 60);


fn run(ui: &mut EgbaUI, gba: &mut GBA, debug: bool) {
    let mut event_pump = ui.get_event_pump().expect("Failed to create SDL2 event pump");
    let mut state = true;

    let mut step_cnt = 0;
    '_game: loop {
        let mut db_frame = false;
        
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    println!("Quit event received. Exiting.");
                    return;
                }
                Event::KeyUp { keycode: Some(keycode), .. } => {
                    match keycode {
                        Keycode::N => {
                            db_frame = true;
                            state = false;
                        },
                        Keycode::P => {
                            state = true;
                        }
                        Keycode::Escape => {
                            println!("Escape key pressed. Exiting.");
                            return;
                        },
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        
        //DEBUG-MODE
        if debug {
            if !db_frame {
                // sleep(Duration::from_millis(100));
            }
            if db_frame || state {
                gba.show_stats();
                gba.step();

                step_cnt += 1;

                println!("{step_cnt}");
            }
        }
        //NORMAL
        else {
            gba.step();
        }

        //KEYPAD
        let keyboard_state = event_pump.keyboard_state();
        let mut keypad = Keypad::default();
        
        for key in keyboard_state.pressed_scancodes().filter_map(Keycode::from_scancode) {
            match key {
                Keycode::A => keypad.a = false,
                Keycode::S => keypad.b = false,
                Keycode::Z => keypad.l = false,
                Keycode::X => keypad.r = false,
                Keycode::Return => keypad.select = false,
                Keycode::Space => keypad.start = false,
                Keycode::Up => keypad.up = false,
                Keycode::Down => keypad.down = false,
                Keycode::Left => keypad.left = false,
                Keycode::Right => keypad.right = false,
                _ => {}
            }
        }

        gba.update_keypad(keypad.into());

        //UPDATE SDL VIDEO
        ui.clear();
        

        //PLAY AUDIO WAVE

        //(REACT HARDWARE) ACCORDING TO DMA, TIMERS, etc...
    }
}

fn main() {
    let args = command!()
        .arg(Arg::new("bios").help("Enter BIOS file path").short('b').long("bios").value_parser(clap::value_parser!(PathBuf)).required(true))
        .arg(Arg::new("rom").help("Enter ROM file path").short('r').long("rom").value_parser(clap::value_parser!(PathBuf)).required(true))
        .arg(Arg::new("backup").help("Enter Backup file path").short('s').long("backup").value_parser(clap::value_parser!(PathBuf)).required(false)).get_matches();

    let bios_path = args.get_one::<PathBuf>("bios").expect("Failed to read BIOS ROM path");
    let bios_buffer = fs::read(bios_path).unwrap();
    let bios_rom = Rom::new(&bios_buffer);
    let bios = Bios::new(bios_rom).unwrap_or_else(|err| {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    });
    
    let rom_path = args.get_one::<PathBuf>("rom").expect("Failed to read Game ROM path");
    let rom_buffer = fs::read(rom_path).unwrap();
    let rom = Rom::new(&rom_buffer);
    
    let backup_path = args.get_one::<PathBuf>("backup").unwrap_or(rom_path).set_extension("sav");
    let cartridge = Cartridge::new(rom, backup_path).unwrap_or_else(|err| {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    });

    let mut egba = GBA::new(bios, cartridge);
    let mut egba_ui = EgbaUI::new().unwrap_or_else(|err| {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    });

    run(&mut egba_ui, &mut egba, true);
}
