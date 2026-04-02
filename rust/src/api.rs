use crate::gameboy::GameBoy;
use crate::joypad::JoypadKey;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{Receiver, Sender, bounded};
#[cfg(target_os = "android")]
use std::panic::{self, AssertUnwindSafe};

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

#[allow(missing_debug_implementations)]
pub struct GameBoyEmulator {
    core: Box<GameBoy>,
    #[allow(dead_code)]
    audio_player: Option<AudioPlayer>,
    audio_tx: Option<Sender<f32>>,
}

struct AudioPlayer {
    _stream: cpal::Stream,
}

struct AudioOutputState {
    rx: Receiver<f32>,
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
    fn new(rx: Receiver<f32>, source_rate: u32, output_rate: u32, channels: usize) -> Self {
        Self {
            rx,
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
        for frame in data.chunks_mut(self.channels) {
            let sample = self.next_output_sample();
            for channel in frame.iter_mut() {
                *channel = sample;
            }
        }
    }

    fn fill_i16_buffer(&mut self, data: &mut [i16]) {
        for frame in data.chunks_mut(self.channels) {
            let sample = (self.next_output_sample() * i16::MAX as f32) as i16;
            for channel in frame.iter_mut() {
                *channel = sample;
            }
        }
    }

    fn fill_u16_buffer(&mut self, data: &mut [u16]) {
        for frame in data.chunks_mut(self.channels) {
            let normalized = (self.next_output_sample() * 0.5) + 0.5;
            let sample = (normalized.clamp(0.0, 1.0) * u16::MAX as f32) as u16;
            for channel in frame.iter_mut() {
                *channel = sample;
            }
        }
    }

    fn next_output_sample(&mut self) -> f32 {
        if !self.primed {
            if self.rx.len() < self.min_buffer_samples {
                return 0.0;
            }
            self.current_sample = self.rx.try_recv().unwrap_or(0.0);
            self.next_sample = self.rx.try_recv().unwrap_or(self.current_sample);
            self.primed = true;
        }

        let sample = self.current_sample
            + ((self.next_sample - self.current_sample) * self.phase.clamp(0.0, 1.0));
        let step = self.source_rate as f32 / self.output_rate as f32;
        self.phase += step;

        while self.phase >= 1.0 {
            self.phase -= 1.0;
            self.current_sample = self.next_sample;
            self.next_sample = self.rx.try_recv().unwrap_or(self.current_sample);
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
            audio_player,
            audio_tx,
        })
    }

    pub fn step_frame(&mut self) {
        self.core.step_frame();

        // Output audio
        if let Some(tx) = &self.audio_tx {
            let samples = self.core.apu.drain_samples();
            for s in samples {
                let _ = tx.try_send(s);
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

    pub fn press_button(&mut self, button: ButtonType) {
        self.core.joypad.set_key(button.into(), true);
    }

    pub fn release_button(&mut self, button: ButtonType) {
        self.core.joypad.set_key(button.into(), false);
    }
}

#[cfg(target_os = "android")]
fn initialize_audio() -> (Option<AudioPlayer>, Option<Sender<f32>>) {
    match panic::catch_unwind(AssertUnwindSafe(initialize_audio_impl)) {
        Ok(result) => result,
        Err(_) => {
            eprintln!("Android audio initialization panicked; continuing without audio output.");
            (None, None)
        }
    }
}

#[cfg(not(target_os = "android"))]
fn initialize_audio() -> (Option<AudioPlayer>, Option<Sender<f32>>) {
    initialize_audio_impl()
}

fn initialize_audio_impl() -> (Option<AudioPlayer>, Option<Sender<f32>>) {
    let host = cpal::default_host();
    let device = host.default_output_device();

    println!("Audio host: {:?}", host.id());
    println!("Default audio device is_some = {:?}", device.is_some());

    let mut audio_player = None;
    let mut audio_tx = None;

    if let Some(dev) = device {
        if let Ok(config) = dev.default_output_config() {
            let sample_format = config.sample_format();
            let sample_rate = config.sample_rate();
            let stream_config = config.config();
            let channels = stream_config.channels as usize;

            println!(
                "Audio Config: sample_rate={}, channels={}",
                sample_rate, channels
            );

            let latency_frames = (sample_rate as f32 * AUDIO_LATENCY_SECONDS) as usize;
            let (tx, rx) = bounded::<f32>(latency_frames * AUDIO_BUFFER_MULTIPLIER);

            let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

            let stream_res = match sample_format {
                cpal::SampleFormat::F32 => {
                    let mut output =
                        AudioOutputState::new(rx, EMULATOR_SAMPLE_RATE, sample_rate, channels);
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
                    let mut output =
                        AudioOutputState::new(rx, EMULATOR_SAMPLE_RATE, sample_rate, channels);
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
                    let mut output =
                        AudioOutputState::new(rx, EMULATOR_SAMPLE_RATE, sample_rate, channels);
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
                    println!("Audio stream built successfully, starting playback");
                    s.play().ok();
                    audio_player = Some(AudioPlayer { _stream: s });
                    audio_tx = Some(tx);
                }
                Err(e) => {
                    println!("Failed to build output stream: {:?}", e);
                }
            }
        } else {
            println!("Failed to get default output config for device");
        }
    } else {
        println!("No audio output device found.");
    }

    (audio_player, audio_tx)
}

#[allow(missing_debug_implementations)]
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
    use super::AudioOutputState;
    use crossbeam_channel::bounded;

    #[test]
    fn duplicates_mono_sample_to_all_output_channels() {
        let (tx, rx) = bounded(8);
        tx.send(0.25).unwrap();
        tx.send(-0.5).unwrap();

        let mut output = AudioOutputState::new(rx, 44_100, 44_100, 2);
        output.min_buffer_samples = 0;
        let mut buffer = [0.0; 4];
        output.fill_f32_buffer(&mut buffer);

        assert_eq!(buffer, [0.25, 0.25, -0.5, -0.5]);
    }

    #[test]
    fn resamples_mono_source_to_higher_output_rate() {
        let (tx, rx) = bounded(8);
        tx.send(-1.0).unwrap();
        tx.send(1.0).unwrap();
        tx.send(-1.0).unwrap();

        let mut output = AudioOutputState::new(rx, 44_100, 48_000, 1);
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
        let (tx, rx) = bounded(4096);
        for _ in 0..2000 {
            tx.send(0.75).unwrap();
        }

        let mut output = AudioOutputState::new(rx, 44_100, 48_000, 1);
        output.min_buffer_samples = 0;
        let mut buffer = [0.0; 2500];
        output.fill_f32_buffer(&mut buffer);

        assert!(buffer.iter().all(|sample| (*sample - 0.75).abs() < 0.001));
    }
}
