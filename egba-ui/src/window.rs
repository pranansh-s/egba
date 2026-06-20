use sdl2::{
    audio::{AudioQueue, AudioSpecDesired},
    pixels::{Color, PixelFormatEnum},
    rect::Rect,
    render::{Canvas, Texture, TextureAccess, TextureCreator},
    video::{Window, WindowContext},
    EventPump, Sdl,
};

const WIDTH: u32 = 240;
const HEIGHT: u32 = 160;
const SCALE: u32 = 3;
const AUDIO_MAX_QUEUED_BYTES: u32 = 8192;

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
    _texture_creator: TextureCreator<WindowContext>,
    texture: Texture,
    dest_rect: Rect,
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
            .present_vsync()
            .build()
            .map_err(|e| EgbaUIError::CanvasCreationError(e.to_string()))?;

        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        canvas.present();

        let texture_creator = canvas.texture_creator();
        let texture = texture_creator
            .create_texture(
                PixelFormatEnum::RGB888,
                TextureAccess::Streaming,
                WIDTH,
                HEIGHT,
            )
            .map_err(|e| EgbaUIError::CanvasCreationError(e.to_string()))?;

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
            _texture_creator: texture_creator,
            texture,
            dest_rect: Rect::new(0, 0, WIDTH * SCALE, HEIGHT * SCALE),
        })
    }

    pub fn get_event_pump(&mut self) -> Result<EventPump, String> {
        self.context.event_pump()
    }

    pub fn render_frame(&mut self, framebuffer: &[u32]) {
        self.texture
            .with_lock(None, |buffer: &mut [u8], pitch: usize| {
                for y in 0..HEIGHT as usize {
                    let row = &mut buffer[y * pitch..y * pitch + (WIDTH as usize) * 4];
                    let src = &framebuffer[y * WIDTH as usize..(y + 1) * WIDTH as usize];
                    for (i, &px) in src.iter().enumerate() {
                        row[i * 4] = (px & 0xFF) as u8;
                        row[i * 4 + 1] = ((px >> 8) & 0xFF) as u8;
                        row[i * 4 + 2] = ((px >> 16) & 0xFF) as u8;
                        row[i * 4 + 3] = 0;
                    }
                }
            })
            .expect("Failed to lock texture");

        self.canvas.clear();
        self.canvas
            .copy(&self.texture, None, Some(self.dest_rect))
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
        if self.audio_device.size() > AUDIO_MAX_QUEUED_BYTES {
            self.audio_device.clear();
        }
        let interleaved: Vec<i16> = samples.iter().flat_map(|&(l, r)| [l, r]).collect();
        let _ = self.audio_device.queue_audio(&interleaved);
    }
}
