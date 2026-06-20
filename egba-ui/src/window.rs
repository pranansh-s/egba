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
const AUDIO_TARGET_QUEUED_BYTES: u32 = 8192;
const AUDIO_DROP_THRESHOLD_BYTES: u32 = 16384;

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
    audio_buf: Vec<i16>,
}

impl EgbaUI {
    pub fn new() -> Result<Self, EgbaUIError> {
        let driver = std::env::var("EGBA_RENDER_DRIVER").unwrap_or_else(|_| "opengl".to_string());
        sdl2::hint::set("SDL_HINT_RENDER_DRIVER", &driver);
        sdl2::hint::set("SDL_RENDER_VSYNC", "0");
        sdl2::hint::set("SDL_HINT_VIDEO_HIGHDPI_DISABLED", "1");
        sdl2::hint::set("SDL_HINT_RENDER_SCALE_QUALITY", "0");
        let context = sdl2::init().map_err(|e| EgbaUIError::SdlInitError(e.to_string()))?;
        let video = context
            .video()
            .map_err(|e| EgbaUIError::VideoInitError(e.to_string()))?;

        let window = video
            .window("EGBA", WIDTH * SCALE, HEIGHT * SCALE)
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
            audio_buf: Vec::with_capacity(4096),
        })
    }

    pub fn get_event_pump(&mut self) -> Result<EventPump, String> {
        self.context.event_pump()
    }

    pub fn render_frame(&mut self, framebuffer: &[u32]) {
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(framebuffer.as_ptr().cast::<u8>(), framebuffer.len() * 4)
        };
        self.texture
            .update(None, bytes, (WIDTH * 4) as usize)
            .expect("Failed to update texture");

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
        let queued = self.audio_device.size();
        if queued > AUDIO_DROP_THRESHOLD_BYTES {
            return;
        }
        let take = if queued > AUDIO_TARGET_QUEUED_BYTES {
            samples.len() / 2
        } else {
            samples.len()
        };
        if take == 0 {
            return;
        }
        self.audio_buf.clear();
        self.audio_buf.reserve(take * 2);
        for &(l, r) in &samples[..take] {
            self.audio_buf.push(l);
            self.audio_buf.push(r);
        }
        let _ = self.audio_device.queue_audio(&self.audio_buf);
    }
}
