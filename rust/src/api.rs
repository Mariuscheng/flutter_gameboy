use crate::cpu::{CpuState, InterruptMasterState};
use crate::gameboy::GameBoy;
use crate::joypad::JoypadKey;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
#[cfg(target_os = "android")]
use std::panic::{self, AssertUnwindSafe};
use std::{
    collections::VecDeque,
    fs,
    sync::{Arc, Mutex},
};

#[cfg(debug_assertions)]
fn log_sync_buttons(
    pressed_mask: u8,
    revision: i32,
    joyp: u8,
    action_keys: u8,
    direction_keys: u8,
    select: u8,
    pc: u16,
    cpu_state: CpuState,
    ime: InterruptMasterState,
    ie: u8,
    iff: u8,
) {
    let _ = (
        pressed_mask,
        revision,
        joyp,
        action_keys,
        direction_keys,
        select,
        pc,
        cpu_state,
        ime,
        ie,
        iff,
    );
}

#[cfg(not(debug_assertions))]
fn log_sync_buttons(
    _pressed_mask: u8,
    _revision: i32,
    _joyp: u8,
    _action_keys: u8,
    _direction_keys: u8,
    _select: u8,
    _pc: u16,
    _cpu_state: CpuState,
    _ime: InterruptMasterState,
    _ie: u8,
    _iff: u8,
) {
}

const EMULATOR_SAMPLE_RATE: u32 = 44_100;
const EMULATOR_FRAME_RATE: f32 = 59.7275;

#[cfg(target_os = "android")]
const AUDIO_LATENCY_SECONDS: f32 = 0.12;
#[cfg(not(target_os = "android"))]
const AUDIO_LATENCY_SECONDS: f32 = 0.05;

#[cfg(target_os = "android")]
const AUDIO_BUFFER_MULTIPLIER: usize = 8;
#[cfg(not(target_os = "android"))]
const AUDIO_BUFFER_MULTIPLIER: usize = 4;

#[cfg(target_os = "android")]
const AUDIO_QUEUE_SECONDS: f32 = 0.35;
#[cfg(not(target_os = "android"))]
const AUDIO_QUEUE_SECONDS: f32 = 0.18;

#[allow(missing_debug_implementations)]
pub struct GameBoyEmulator {
    core: Box<GameBoy>,
    _audio_player: Option<AudioPlayer>,
    audio_buffer: Option<SharedAudioBufferHandle>,
    last_input_revision: i32,
}

struct AudioPlayer {
    _stream: cpal::Stream,
}

type SharedAudioBufferHandle = Arc<Mutex<SharedAudioBuffer>>;

struct SharedAudioBuffer {
    samples: VecDeque<f32>,
    max_samples: usize,
}

impl SharedAudioBuffer {
    fn new(max_samples: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_samples),
            max_samples,
        }
    }

    fn len(&self) -> usize {
        self.samples.len()
    }

    fn pop_front(&mut self) -> Option<f32> {
        self.samples.pop_front()
    }

    fn push_samples(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }

        if samples.len() >= self.max_samples {
            self.samples.clear();
            self.samples.extend(
                samples[samples.len().saturating_sub(self.max_samples)..]
                    .iter()
                    .copied(),
            );
            return;
        }

        let overflow = self
            .samples
            .len()
            .saturating_add(samples.len())
            .saturating_sub(self.max_samples);
        if overflow > 0 {
            self.samples.drain(..overflow);
        }

        self.samples.extend(samples.iter().copied());
    }
}

struct AudioOutputState {
    shared_buffer: SharedAudioBufferHandle,
    source_rate: u32,
    output_rate: u32,
    channels: usize,
    min_buffer_samples: usize,
    phase: f32,
    current_sample: f32,
    next_sample: f32,
    primed: bool,
}

impl AudioOutputState {
    fn new(
        shared_buffer: SharedAudioBufferHandle,
        source_rate: u32,
        output_rate: u32,
        channels: usize,
    ) -> Self {
        Self {
            shared_buffer,
            source_rate,
            output_rate,
            channels,
            min_buffer_samples: ((source_rate as f32 / EMULATOR_FRAME_RATE) * 2.0) as usize,
            phase: 0.0,
            current_sample: 0.0,
            next_sample: 0.0,
            primed: false,
        }
    }

    fn fill_f32_buffer(&mut self, data: &mut [f32]) {
        let shared_buffer_handle = Arc::clone(&self.shared_buffer);
        let mut shared_buffer = shared_buffer_handle.lock().unwrap();
        for frame in data.chunks_mut(self.channels) {
            let sample = self.next_output_sample(&mut shared_buffer);
            for channel in frame.iter_mut() {
                *channel = sample;
            }
        }
    }

    fn fill_i16_buffer(&mut self, data: &mut [i16]) {
        let shared_buffer_handle = Arc::clone(&self.shared_buffer);
        let mut shared_buffer = shared_buffer_handle.lock().unwrap();
        for frame in data.chunks_mut(self.channels) {
            let sample = (self.next_output_sample(&mut shared_buffer) * i16::MAX as f32) as i16;
            for channel in frame.iter_mut() {
                *channel = sample;
            }
        }
    }

    fn fill_u16_buffer(&mut self, data: &mut [u16]) {
        let shared_buffer_handle = Arc::clone(&self.shared_buffer);
        let mut shared_buffer = shared_buffer_handle.lock().unwrap();
        for frame in data.chunks_mut(self.channels) {
            let normalized = (self.next_output_sample(&mut shared_buffer) * 0.5) + 0.5;
            let sample = (normalized.clamp(0.0, 1.0) * u16::MAX as f32) as u16;
            for channel in frame.iter_mut() {
                *channel = sample;
            }
        }
    }

    fn next_output_sample(&mut self, shared_buffer: &mut SharedAudioBuffer) -> f32 {
        if !self.primed {
            if shared_buffer.len() < self.min_buffer_samples {
                return 0.0;
            }
            self.current_sample = shared_buffer.pop_front().unwrap_or(0.0);
            self.next_sample = shared_buffer.pop_front().unwrap_or(self.current_sample);
            self.primed = true;
        }

        let sample = self.current_sample
            + ((self.next_sample - self.current_sample) * self.phase.clamp(0.0, 1.0));
        let step = self.source_rate as f32 / self.output_rate as f32;
        self.phase += step;

        while self.phase >= 1.0 {
            self.phase -= 1.0;
            self.current_sample = self.next_sample;
            self.next_sample = shared_buffer.pop_front().unwrap_or(self.current_sample);
        }

        sample.clamp(-1.0, 1.0)
    }
}

unsafe impl Send for GameBoyEmulator {}
unsafe impl Sync for GameBoyEmulator {}

impl GameBoyEmulator {
    pub fn new(rom_bytes: Vec<u8>) -> Result<Self, String> {
        let mut gb = GameBoy::new();
        gb.load_rom_from_bytes(rom_bytes)
            .map_err(|e| e.to_string())?;

        let (audio_player, audio_tx) = initialize_audio();

        Ok(Self {
            core: gb,
            _audio_player: audio_player,
            audio_buffer: audio_tx,
            last_input_revision: 0,
        })
    }

    pub fn new_from_path(path: String) -> Result<Self, String> {
        let rom_bytes =
            fs::read(&path).map_err(|error| format!("Failed to read ROM '{}': {}", path, error))?;
        Self::new(rom_bytes)
    }

    pub fn step_frame(&mut self) {
        self.core.step_frame();

        // Output audio
        if let Some(buffer) = &self.audio_buffer {
            let samples = self.core.apu.drain_samples();
            if !samples.is_empty() {
                if let Ok(mut shared_buffer) = buffer.lock() {
                    shared_buffer.push_samples(&samples);
                }
            }
        }
    }

    pub fn get_frame_buffer(&self) -> Vec<u8> {
        let present = self.core.get_present_framebuffer();
        let mut rgba = Vec::with_capacity(160 * 144 * 4);
        for &pixel in present {
            let (r, g, b) = match pixel {
                0 => (155, 188, 15), // Lightest
                1 => (139, 172, 15), // Light gray
                2 => (48, 98, 48),   // Dark gray
                3 => (15, 56, 15),   // Darkest
                _ => (0, 0, 0),
            };
            rgba.push(r);
            rgba.push(g);
            rgba.push(b);
            rgba.push(255); // Alpha
        }
        rgba
    }

    fn apply_button_state(&mut self, button: ButtonType, pressed: bool) {
        // Joypad internally calls trigger_interrupt() if edge detected,
        // which accumulates into self.core.interrupt_handler.if_register.
        self.core.joypad.set_key(button.into(), pressed);
    }

    pub fn press_button(&mut self, button: ButtonType) {
        self.apply_button_state(button, true);
    }

    pub fn release_button(&mut self, button: ButtonType) {
        self.apply_button_state(button, false);
    }

    pub fn sync_buttons(&mut self, pressed_mask: u8, revision: i32) {
        if revision < self.last_input_revision {
            return;
        }
        self.last_input_revision = revision;

        for button in [
            ButtonType::A,
            ButtonType::B,
            ButtonType::Start,
            ButtonType::Select,
            ButtonType::Up,
            ButtonType::Down,
            ButtonType::Left,
            ButtonType::Right,
        ] {
            let mask = button.mask();
            self.apply_button_state(button, (pressed_mask & mask) != 0);
        }

        log_sync_buttons(
            pressed_mask,
            revision,
            self.core.joypad.read_register(),
            self.core.joypad.action_keys,
            self.core.joypad.direction_keys,
            self.core.joypad.select,
            self.core.cpu.pc,
            self.core.cpu.state,
            self.core.cpu.ime,
            self.core.mmu.ie,
            self.core.mmu.read_byte(0xFF0F),
        );
    }
}

#[cfg(target_os = "android")]
fn initialize_audio() -> (Option<AudioPlayer>, Option<SharedAudioBufferHandle>) {
    match panic::catch_unwind(AssertUnwindSafe(initialize_audio_impl)) {
        Ok(result) => result,
        Err(_) => {
            eprintln!("Android audio initialization panicked; continuing without audio output.");
            (None, None)
        }
    }
}

#[cfg(not(target_os = "android"))]
fn initialize_audio() -> (Option<AudioPlayer>, Option<SharedAudioBufferHandle>) {
    initialize_audio_impl()
}

fn initialize_audio_impl() -> (Option<AudioPlayer>, Option<SharedAudioBufferHandle>) {
    let host = cpal::default_host();
    let device = host.default_output_device();
    let _ = host.id();

    let mut audio_player = None;
    let mut audio_buffer = None;

    if let Some(dev) = device {
        if let Ok(config) = dev.default_output_config() {
            let sample_format = config.sample_format();
            let sample_rate = config.sample_rate();
            let stream_config = config.config();
            let channels = stream_config.channels as usize;

            let latency_frames = (sample_rate as f32 * AUDIO_LATENCY_SECONDS) as usize;
            let max_samples = ((EMULATOR_SAMPLE_RATE as f32 * AUDIO_QUEUE_SECONDS) as usize)
                .max(latency_frames * AUDIO_BUFFER_MULTIPLIER);
            let shared_buffer = Arc::new(Mutex::new(SharedAudioBuffer::new(max_samples)));

            let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

            let stream_res = match sample_format {
                cpal::SampleFormat::F32 => {
                    let mut output = AudioOutputState::new(
                        Arc::clone(&shared_buffer),
                        EMULATOR_SAMPLE_RATE,
                        sample_rate,
                        channels,
                    );
                    dev.build_output_stream(
                        &stream_config,
                        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                            output.fill_f32_buffer(data);
                        },
                        err_fn,
                        None,
                    )
                }
                cpal::SampleFormat::I16 => {
                    let mut output = AudioOutputState::new(
                        Arc::clone(&shared_buffer),
                        EMULATOR_SAMPLE_RATE,
                        sample_rate,
                        channels,
                    );
                    dev.build_output_stream(
                        &stream_config,
                        move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                            output.fill_i16_buffer(data);
                        },
                        err_fn,
                        None,
                    )
                }
                cpal::SampleFormat::U16 => {
                    let mut output = AudioOutputState::new(
                        Arc::clone(&shared_buffer),
                        EMULATOR_SAMPLE_RATE,
                        sample_rate,
                        channels,
                    );
                    dev.build_output_stream(
                        &stream_config,
                        move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                            output.fill_u16_buffer(data);
                        },
                        err_fn,
                        None,
                    )
                }
                _ => Err(cpal::BuildStreamError::StreamConfigNotSupported),
            };

            match stream_res {
                Ok(s) => {
                    s.play().ok();
                    audio_player = Some(AudioPlayer { _stream: s });
                    audio_buffer = Some(shared_buffer);
                }
                Err(_e) => {}
            }
        }
    }

    (audio_player, audio_buffer)
}

#[allow(missing_debug_implementations)]
#[derive(Clone, Copy)]
pub enum ButtonType {
    A,
    B,
    Start,
    Select,
    Up,
    Down,
    Left,
    Right,
}

impl ButtonType {
    fn mask(self) -> u8 {
        match self {
            ButtonType::A => 1 << 0,
            ButtonType::B => 1 << 1,
            ButtonType::Start => 1 << 2,
            ButtonType::Select => 1 << 3,
            ButtonType::Up => 1 << 4,
            ButtonType::Down => 1 << 5,
            ButtonType::Left => 1 << 6,
            ButtonType::Right => 1 << 7,
        }
    }
}

impl From<ButtonType> for JoypadKey {
    fn from(btn: ButtonType) -> Self {
        match btn {
            ButtonType::A => JoypadKey::A,
            ButtonType::B => JoypadKey::B,
            ButtonType::Start => JoypadKey::Start,
            ButtonType::Select => JoypadKey::Select,
            ButtonType::Up => JoypadKey::Up,
            ButtonType::Down => JoypadKey::Down,
            ButtonType::Left => JoypadKey::Left,
            ButtonType::Right => JoypadKey::Right,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AudioOutputState, SharedAudioBuffer, SharedAudioBufferHandle};
    use std::sync::{Arc, Mutex};

    fn shared_buffer_with_samples(samples: &[f32], max_samples: usize) -> SharedAudioBufferHandle {
        let buffer = Arc::new(Mutex::new(SharedAudioBuffer::new(max_samples)));
        buffer.lock().unwrap().push_samples(samples);
        buffer
    }

    #[test]
    fn duplicates_mono_sample_to_all_output_channels() {
        let shared_buffer = shared_buffer_with_samples(&[0.25, -0.5], 8);
        let mut output = AudioOutputState::new(shared_buffer, 44_100, 44_100, 2);
        output.min_buffer_samples = 0;
        let mut buffer = [0.0; 4];
        output.fill_f32_buffer(&mut buffer);

        assert_eq!(buffer, [0.25, 0.25, -0.5, -0.5]);
    }

    #[test]
    fn resamples_mono_source_to_higher_output_rate() {
        let shared_buffer = shared_buffer_with_samples(&[-1.0, 1.0, -1.0], 8);
        let mut output = AudioOutputState::new(shared_buffer, 44_100, 48_000, 1);
        output.min_buffer_samples = 0;
        let mut buffer = [0.0; 4];
        output.fill_f32_buffer(&mut buffer);

        assert!(buffer[0] <= -0.99);
        assert!(buffer[1] > 0.8);
        assert!(buffer[2] < 0.0);
        assert!(buffer[3].abs() <= 1.0);
        assert!(buffer.iter().all(|sample| (-1.0..=1.0).contains(sample)));
    }

    #[test]
    fn holds_last_sample_when_source_buffer_runs_dry() {
        let samples = vec![0.75; 2000];
        let shared_buffer = shared_buffer_with_samples(&samples, 4096);
        let mut output = AudioOutputState::new(shared_buffer, 44_100, 48_000, 1);
        output.min_buffer_samples = 0;
        let mut buffer = [0.0; 2500];
        output.fill_f32_buffer(&mut buffer);

        assert!(buffer.iter().all(|sample| (*sample - 0.75).abs() < 0.001));
    }
}
