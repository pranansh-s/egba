use sdl2::{
    keyboard::Keycode, pixels::Color, rect::Point, render::Canvas, video::Window, AudioSubsystem,
    EventPump, Sdl,
};

const WIDTH: usize = 240;
const HEIGHT: usize = 160;

use std::{error::Error, fmt};

pub fn get_keystate(event_pump: &EventPump) -> u16 {
    let mut keystate = 0xFFFF;
    let keyboard_state = event_pump.keyboard_state();
    for key in keyboard_state
        .pressed_scancodes()
        .filter_map(Keycode::from_scancode)
    {
        match key {
            Keycode::A => keystate &= !(1 << 0),
            Keycode::S => keystate &= !(1 << 1),
            Keycode::Z => keystate &= !(1 << 2),
            Keycode::X => keystate &= !(1 << 3),
            Keycode::Return => keystate &= !(1 << 4),
            Keycode::Space => keystate &= !(1 << 5),
            Keycode::Up => keystate &= !(1 << 6),
            Keycode::Down => keystate &= !(1 << 7),
            Keycode::Left => keystate &= !(1 << 8),
            Keycode::Right => keystate &= !(1 << 9),
            _ => {}
        }
    }
    keystate
}

#[derive(Debug)]
pub enum EgbaUIError {
    SdlInitError(String),
    VideoInitError(String),
    AudioInitError(String),
    WindowCreationError(String),
    CanvasCreationError(String),
    ContextInitError(String),
}

impl fmt::Display for EgbaUIError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EgbaUIError::SdlInitError(e) => write!(f, "SDL initialization error: {}", e),
            EgbaUIError::VideoInitError(e) => write!(f, "SDL video initialization error: {}", e),
            EgbaUIError::AudioInitError(e) => write!(f, "SDL audio initialization error: {}", e),
            EgbaUIError::WindowCreationError(e) => write!(f, "Window creation error: {}", e),
            EgbaUIError::CanvasCreationError(e) => write!(f, "Canvas creation error: {}", e),
            EgbaUIError::ContextInitError(e) => {
                write!(f, "SDL context initialization error: {}", e)
            }
        }
    }
}

impl Error for EgbaUIError {}

pub struct EgbaUI {
    canvas: Canvas<Window>,
    context: Sdl,
    audio: AudioSubsystem,
}

impl EgbaUI {
    pub fn new() -> Result<Self, EgbaUIError> {
        let context = sdl2::init().map_err(|e| EgbaUIError::SdlInitError(e.to_string()))?;
        let video = context
            .video()
            .map_err(|e| EgbaUIError::VideoInitError(e.to_string()))?;
        let audio = context
            .audio()
            .map_err(|e| EgbaUIError::AudioInitError(e.to_string()))?;

        let window = video
            .window("EGBA", WIDTH as u32, HEIGHT as u32)
            .opengl()
            .position_centered()
            .allow_highdpi()
            .build()
            .map_err(|e| EgbaUIError::WindowCreationError(e.to_string()))?;
        let mut canvas = window
            .into_canvas()
            .accelerated()
            .present_vsync()
            .build()
            .map_err(|e| EgbaUIError::CanvasCreationError(e.to_string()))?;
        let context = sdl2::init().map_err(|e| EgbaUIError::ContextInitError(e))?;

        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        canvas.present();

        Ok(Self {
            canvas,
            context,
            audio,
        })
    }

    pub fn get_event_pump(&mut self) -> Result<EventPump, String> {
        self.context.event_pump()
    }

    pub fn draw_pixel(&mut self, x: u32, y: u32, color: Color) {
        let point = Point::new(x as i32, y as i32);

        self.canvas.set_draw_color(color);
        self.canvas
            .draw_point(point)
            .expect(&format!("Failed to draw pixel at ({}, {})", x, y));
        self.canvas.present();
    }

    pub fn clear(&mut self) {
        self.canvas.set_draw_color(Color::RGB(0, 0, 0));
        self.canvas.clear();
    }
}
