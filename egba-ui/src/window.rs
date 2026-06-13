use sdl2::{
    audio::{AudioQueue, AudioSpecDesired},
    pixels::{Color, PixelFormatEnum},
    rect::Rect,
    render::{Canvas, TextureAccess},
    video::Window,
    EventPump, Sdl,
};

const WIDTH: u32 = 240;
const HEIGHT: u32 = 160;
const SCALE: u32 = 3;

use std::{error::Error, fmt};

use sdl2::keyboard::Scancode;

pub fn get_keystate(event_pump: &EventPump) -> u16 {
    let mut keystate = 0xFFFF;
    let keyboard_state = event_pump.keyboard_state();

    if keyboard_state.is_scancode_pressed(Scancode::A) {
        keystate &= !(1 << 0);
    }
    if keyboard_state.is_scancode_pressed(Scancode::S) {
        keystate &= !(1 << 1);
    }
    if keyboard_state.is_scancode_pressed(Scancode::Z) {
        keystate &= !(1 << 2);
    }
    if keyboard_state.is_scancode_pressed(Scancode::X) {
        keystate &= !(1 << 3);
    }
    if keyboard_state.is_scancode_pressed(Scancode::Return) {
        keystate &= !(1 << 4);
    }
    if keyboard_state.is_scancode_pressed(Scancode::Space) {
        keystate &= !(1 << 5);
    }
    if keyboard_state.is_scancode_pressed(Scancode::Up) {
        keystate &= !(1 << 6);
    }
    if keyboard_state.is_scancode_pressed(Scancode::Down) {
        keystate &= !(1 << 7);
    }
    if keyboard_state.is_scancode_pressed(Scancode::Left) {
        keystate &= !(1 << 8);
    }
    if keyboard_state.is_scancode_pressed(Scancode::Right) {
        keystate &= !(1 << 9);
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
    audio_device: AudioQueue<i16>,
}

impl EgbaUI {
    pub fn new() -> Result<Self, EgbaUIError> {
        let context = sdl2::init().map_err(|e| EgbaUIError::SdlInitError(e.to_string()))?;
        let video = context
            .video()
            .map_err(|e| EgbaUIError::VideoInitError(e.to_string()))?;

        let window = video
            .window("EGBA", WIDTH * SCALE, HEIGHT * SCALE)
            .opengl()
            .position_centered()
            .build()
            .map_err(|e| EgbaUIError::WindowCreationError(e.to_string()))?;
        let mut canvas = window
            .into_canvas()
            .accelerated()
            .build()
            .map_err(|e| EgbaUIError::CanvasCreationError(e.to_string()))?;

        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        canvas.present();

        let audio_subsystem = context
            .audio()
            .map_err(|e| EgbaUIError::AudioInitError(e.to_string()))?;

        let desired_spec = AudioSpecDesired {
            freq: Some(32768),
            channels: Some(2),
            samples: Some(512),
        };

        let audio_device = audio_subsystem
            .open_queue::<i16, _>(None, &desired_spec)
            .map_err(|e| EgbaUIError::AudioInitError(e.to_string()))?;

        audio_device.resume();

        Ok(Self {
            canvas,
            context,
            audio_device,
        })
    }

    pub fn get_event_pump(&mut self) -> Result<EventPump, String> {
        self.context.event_pump()
    }

    pub fn render_frame(&mut self, framebuffer: &[u32]) {
        let texture_creator = self.canvas.texture_creator();
        let mut texture = texture_creator
            .create_texture(
                PixelFormatEnum::RGB888,
                TextureAccess::Streaming,
                WIDTH,
                HEIGHT,
            )
            .expect("Failed to create texture");

        let pixel_data: Vec<u8> = framebuffer
            .iter()
            .flat_map(|&pixel| {
                let r = ((pixel >> 16) & 0xFF) as u8;
                let g = ((pixel >> 8) & 0xFF) as u8;
                let b = (pixel & 0xFF) as u8;
                [0u8, r, g, b]
            })
            .collect();

        texture
            .update(None, &pixel_data, (WIDTH * 4) as usize)
            .expect("Failed to update texture");

        self.canvas.clear();
        self.canvas
            .copy(
                &texture,
                None,
                Some(Rect::new(0, 0, WIDTH * SCALE, HEIGHT * SCALE)),
            )
            .expect("Failed to copy texture to canvas");
        self.canvas.present();
    }

    pub fn clear(&mut self) {
        self.canvas.set_draw_color(Color::RGB(0, 0, 0));
        self.canvas.clear();
    }

    pub fn queue_audio(&mut self, samples: &[(i16, i16)]) {
        if samples.is_empty() {
            return;
        }
        let interleaved: Vec<i16> = samples
            .iter()
            .flat_map(|&(l, r)| [l, r])
            .collect();
        let _ = self.audio_device.queue_audio(&interleaved);
    }
}
