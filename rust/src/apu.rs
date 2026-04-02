use std::collections::VecDeque;

// APU (Audio Processing Unit) - Game Boy 音訊處理器
pub struct Apu {
    // 方波通道 A (Pulse A) - 0xFF10-0xFF14
    pulse_a: PulseChannel,
    // 方波通道 B (Pulse B) - 0xFF16-0xFF19
    pulse_b: PulseChannel,
    // 波形通道 (Wave) - 0xFF1A-0xFF1E, 0xFF30-0xFF3F
    wave: WaveChannel,
    // 噪音通道 (Noise) - 0xFF20-0xFF23
    noise: NoiseChannel,

    // 控制寄存器
    nr50: u8, // 0xFF24 - 主音量
    nr51: u8, // 0xFF25 - 聲音輸出選擇 (panning)
    nr52: u8, // 0xFF26 - 聲音控制/狀態

    // 幀序列器 - 512Hz 時鐘
    frame_sequencer: FrameSequencer,

    // 音訊緩衝區
    pub audio_buffer: VecDeque<f32>,
    sample_counter: u32,
}

// 使用整數算術避免浮點數漂移
// CPU 頻率 4194304 Hz，樣本率 44100 Hz
// 每 95.0996 個週期一個樣本
// 使用 4194304 / 44100 = 95 + 1/11.666...
// 我們使用兩個計數器來實現精確的定時
const SAMPLE_NUMERATOR: u32 = 4194304;
const SAMPLE_DENOMINATOR: u32 = 44100;

impl Apu {
    pub fn new() -> Self {
        Apu {
            pulse_a: PulseChannel::new(true),  // 有 sweep
            pulse_b: PulseChannel::new(false), // 無 sweep
            wave: WaveChannel::new(),
            noise: NoiseChannel::new(),
            nr50: 0,
            nr51: 0,
            nr52: 0,
            frame_sequencer: FrameSequencer::new(),
            audio_buffer: VecDeque::with_capacity(8192),
            sample_counter: 0,
        }
    }

    // 讀取 APU 寄存器
    pub fn read_register(&self, addr: u16) -> u8 {
        // DMG: 即使 APU 關閉也允許讀取寄存器
        // 只是寄存器的值會被清零（power_off 時）
        match addr {
            0xFF10 => self.pulse_a.read_nr10() | 0x80,
            0xFF11 => self.pulse_a.read_nr11() | 0x3F,
            0xFF12 => self.pulse_a.read_nr12(),
            0xFF13 => 0xFF, // NR13 是只寫的
            0xFF14 => self.pulse_a.read_nr14() | 0xBF,

            0xFF15 => 0xFF, // NR20 不存在
            0xFF16 => self.pulse_b.read_nr11() | 0x3F,
            0xFF17 => self.pulse_b.read_nr12(),
            0xFF18 => 0xFF, // NR23 是只寫的
            0xFF19 => self.pulse_b.read_nr14() | 0xBF,

            0xFF1A => self.wave.read_nr30() | 0x7F,
            0xFF1B => 0xFF, // NR31 是只寫的
            0xFF1C => self.wave.read_nr32() | 0x9F,
            0xFF1D => 0xFF, // NR33 是只寫的
            0xFF1E => self.wave.read_nr34() | 0xBF,

            0xFF1F => 0xFF, // NR40 不存在
            0xFF20 => 0xFF, // NR41 是只寫的
            0xFF21 => self.noise.read_nr42(),
            0xFF22 => self.noise.read_nr43(),
            0xFF23 => self.noise.read_nr44() | 0xBF,

            0xFF24 => self.nr50,
            0xFF25 => self.nr51,
            0xFF26 => {
                let mut status = 0x70; // 位 4-6 始終為 1
                if self.is_enabled() {
                    status |= 0x80;
                }
                if self.pulse_a.is_enabled() {
                    status |= 0x01;
                }
                if self.pulse_b.is_enabled() {
                    status |= 0x02;
                }
                if self.wave.is_enabled() {
                    status |= 0x04;
                }
                if self.noise.is_enabled() {
                    status |= 0x08;
                }
                status
            }

            0xFF30..=0xFF3F => self.wave.read_wave_ram(addr - 0xFF30),
            _ => 0xFF,
        }
    }

    // 寫入 APU 寄存器
    pub fn write_register(&mut self, addr: u16, value: u8) {
        // Wave RAM 可以隨時寫入
        if (0xFF30..=0xFF3F).contains(&addr) {
            self.wave.write_wave_ram(addr - 0xFF30, value);
            return;
        }

        // NR52 可以隨時寫入
        if addr == 0xFF26 {
            let was_enabled = self.is_enabled();
            let now_enabled = (value & 0x80) != 0;

            if was_enabled && !now_enabled {
                // 關閉 APU - 重置所有寄存器
                self.power_off();
            } else if !was_enabled && now_enabled {
                // 開啟 APU
                self.frame_sequencer.reset();
            }

            self.nr52 = value & 0x80;
            return;
        }

        // 如果 APU 關閉，忽略其他寫入（除了長度計數器）
        if !self.is_enabled() {
            // DMG 允許在 APU 關閉時寫入長度計數器
            match addr {
                0xFF11 => self.pulse_a.write_nr11_length_only(value),
                0xFF16 => self.pulse_b.write_nr11_length_only(value),
                0xFF1B => self.wave.write_nr31(value),
                0xFF20 => self.noise.write_nr41(value),
                _ => {}
            }
            return;
        }

        match addr {
            0xFF10 => self.pulse_a.write_nr10(value),
            0xFF11 => self.pulse_a.write_nr11(value),
            0xFF12 => self.pulse_a.write_nr12(value),
            0xFF13 => self.pulse_a.write_nr13(value),
            0xFF14 => self.pulse_a.write_nr14(value, self.frame_sequencer.step),

            0xFF15 => {} // NR20 不存在
            0xFF16 => self.pulse_b.write_nr11(value),
            0xFF17 => self.pulse_b.write_nr12(value),
            0xFF18 => self.pulse_b.write_nr13(value),
            0xFF19 => self.pulse_b.write_nr14(value, self.frame_sequencer.step),

            0xFF1A => self.wave.write_nr30(value),
            0xFF1B => self.wave.write_nr31(value),
            0xFF1C => self.wave.write_nr32(value),
            0xFF1D => self.wave.write_nr33(value),
            0xFF1E => self.wave.write_nr34(value, self.frame_sequencer.step),

            0xFF1F => {} // NR40 不存在
            0xFF20 => self.noise.write_nr41(value),
            0xFF21 => self.noise.write_nr42(value),
            0xFF22 => self.noise.write_nr43(value),
            0xFF23 => self.noise.write_nr44(value, self.frame_sequencer.step),

            0xFF24 => self.nr50 = value,
            0xFF25 => self.nr51 = value,

            _ => {}
        }
    }

    fn is_enabled(&self) -> bool {
        (self.nr52 & 0x80) != 0
    }

    fn power_off(&mut self) {
        self.pulse_a.power_off();
        self.pulse_b.power_off();
        self.wave.power_off();
        self.noise.power_off();
        self.nr50 = 0;
        self.nr51 = 0;
        self.sample_counter = 0; // 重置樣本計數器
    }

    // 更新 APU 狀態 (每 T-cycle 調用)
    pub fn tick(&mut self) {
        if !self.is_enabled() {
            return;
        }

        // 幀序列器 - 每 8192 T-cycles (512Hz)
        // step 在 tick() 中已經遞增，所以 current_step() 返回的是新的步驟
        if self.frame_sequencer.tick() {
            // Frame sequencer steps:
            // Step 0: Length
            // Step 1: -
            // Step 2: Length, Sweep
            // Step 3: -
            // Step 4: Length
            // Step 5: -
            // Step 6: Length, Sweep
            // Step 7: Envelope
            let step = self.frame_sequencer.current_step();

            // Length clocks on steps 0, 2, 4, 6 (even steps)
            if step.is_multiple_of(2) {
                self.clock_length();
            }

            // Sweep clocks on steps 2, 6
            if step == 2 || step == 6 {
                self.pulse_a.clock_sweep();
            }

            // Envelope clocks on step 7
            if step == 7 {
                self.clock_envelope();
            }
        }

        // 頻率計時器
        self.pulse_a.tick();
        self.pulse_b.tick();
        self.wave.tick();
        self.noise.tick();

        // 採樣 - 使用整數算術避免漂移
        // CPU 頻率 4194304Hz，樣本率 44100Hz
        self.sample_counter += SAMPLE_DENOMINATOR;
        if self.sample_counter >= SAMPLE_NUMERATOR {
            self.sample_counter -= SAMPLE_NUMERATOR;
            self.output_sample();
        }
    }

    fn clock_length(&mut self) {
        self.pulse_a.clock_length();
        self.pulse_b.clock_length();
        self.wave.clock_length();
        self.noise.clock_length();
    }

    fn clock_envelope(&mut self) {
        self.pulse_a.clock_envelope();
        self.pulse_b.clock_envelope();
        self.noise.clock_envelope();
    }

    fn output_sample(&mut self) {
        let (left, right) = self.mix_channels();
        // 混合左右聲道為單聲道（與 SDL 設定匹配）
        let mono = (left + right) * 0.5;
        self.audio_buffer.push_back(mono);
    }

    fn mix_channels(&self) -> (f32, f32) {
        // 獲取各通道的 DAC 輸出
        let ch1 = self.pulse_a.get_output();
        let ch2 = self.pulse_b.get_output();
        let ch3 = self.wave.get_output();
        let ch4 = self.noise.get_output();

        // 混合到左右聲道（根據 NR51）
        let mut left = 0.0f32;
        let mut right = 0.0f32;

        if (self.nr51 & 0x10) != 0 {
            left += ch1;
        }
        if (self.nr51 & 0x20) != 0 {
            left += ch2;
        }
        if (self.nr51 & 0x40) != 0 {
            left += ch3;
        }
        if (self.nr51 & 0x80) != 0 {
            left += ch4;
        }

        if (self.nr51 & 0x01) != 0 {
            right += ch1;
        }
        if (self.nr51 & 0x02) != 0 {
            right += ch2;
        }
        if (self.nr51 & 0x04) != 0 {
            right += ch3;
        }
        if (self.nr51 & 0x08) != 0 {
            right += ch4;
        }

        // 應用主音量 (NR50)
        let left_vol = ((self.nr50 >> 4) & 0x07) as f32 + 1.0;
        let right_vol = (self.nr50 & 0x07) as f32 + 1.0;

        left = left * left_vol / 32.0;
        right = right * right_vol / 32.0;

        // 正規化到 [-1.0, 1.0]
        (left.clamp(-1.0, 1.0), right.clamp(-1.0, 1.0))
    }

    pub fn drain_samples(&mut self) -> Vec<f32> {
        self.audio_buffer.drain(..).collect()
    }
}

// 幀序列器 - 512Hz 時鐘
struct FrameSequencer {
    timer: u16,
    step: u8,
}

impl FrameSequencer {
    fn new() -> Self {
        FrameSequencer { timer: 0, step: 0 }
    }

    fn reset(&mut self) {
        // APU 開啟時，timer 設為 0，這樣 8192 T-cycles 後會發生第一個事件
        // step 設為 7，這樣第一個事件會將其變為 0（執行 length clock）
        self.timer = 0;
        self.step = 7;
    }

    fn tick(&mut self) -> bool {
        self.timer = self.timer.wrapping_add(1);
        if self.timer >= 8192 {
            self.timer = 0;
            self.step = (self.step + 1) & 7;
            true
        } else {
            false
        }
    }

    fn current_step(&self) -> u8 {
        // 返回當前步驟（tick 後的步驟）
        self.step
    }
}

// 方波通道
struct PulseChannel {
    enabled: bool,
    dac_enabled: bool,

    // 頻率
    frequency: u16,
    frequency_timer: u16,

    // 占空比
    duty: u8,
    duty_position: u8,

    // 長度計數器
    length_counter: u16,
    length_enabled: bool,

    // 包絡
    envelope_volume: u8,
    envelope_direction: bool, // true = 增加
    envelope_period: u8,
    envelope_timer: u8,
    current_volume: u8,

    // 掃描 (僅 Pulse A)
    has_sweep: bool,
    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift: u8,
    sweep_timer: u8,
    sweep_enabled: bool,
    sweep_shadow: u16,
    sweep_negate_used: bool,

    // 暫存寄存器值
    nr10: u8,
    nr11: u8,
    nr12: u8,
}

impl PulseChannel {
    fn new(has_sweep: bool) -> Self {
        PulseChannel {
            enabled: false,
            dac_enabled: false,
            frequency: 0,
            frequency_timer: 0,
            duty: 0,
            duty_position: 0,
            length_counter: 0,
            length_enabled: false,
            envelope_volume: 0,
            envelope_direction: false,
            envelope_period: 0,
            envelope_timer: 0,
            current_volume: 0,
            has_sweep,
            sweep_period: 0,
            sweep_negate: false,
            sweep_shift: 0,
            sweep_timer: 0,
            sweep_enabled: false,
            sweep_shadow: 0,
            sweep_negate_used: false,
            nr10: 0,
            nr11: 0,
            nr12: 0,
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn power_off(&mut self) {
        self.enabled = false;
        self.dac_enabled = false;
        self.frequency = 0;
        self.duty = 0;
        self.length_enabled = false;
        self.envelope_volume = 0;
        self.envelope_direction = false;
        self.envelope_period = 0;
        self.current_volume = 0;
        self.sweep_period = 0;
        self.sweep_negate = false;
        self.sweep_shift = 0;
        self.nr10 = 0;
        self.nr11 = 0;
        self.nr12 = 0;
    }

    fn read_nr10(&self) -> u8 {
        self.nr10
    }

    fn read_nr11(&self) -> u8 {
        self.nr11 & 0xC0 // 只返回 duty
    }

    fn read_nr12(&self) -> u8 {
        self.nr12
    }

    fn read_nr14(&self) -> u8 {
        if self.length_enabled { 0x40 } else { 0x00 }
    }

    fn write_nr10(&mut self, value: u8) {
        self.nr10 = value;
        self.sweep_period = (value >> 4) & 0x07;
        let new_negate = (value & 0x08) != 0;

        // 如果之前使用了 negate，現在切換到非 negate，則禁用通道
        if self.sweep_negate_used && !new_negate {
            self.enabled = false;
        }

        self.sweep_negate = new_negate;
        self.sweep_shift = value & 0x07;
    }

    fn write_nr11(&mut self, value: u8) {
        self.nr11 = value;
        self.duty = (value >> 6) & 0x03;
        self.length_counter = 64 - (value & 0x3F) as u16;
    }

    fn write_nr11_length_only(&mut self, value: u8) {
        self.length_counter = 64 - (value & 0x3F) as u16;
    }

    fn write_nr12(&mut self, value: u8) {
        self.nr12 = value;
        self.envelope_volume = value >> 4;
        self.envelope_direction = (value & 0x08) != 0;
        self.envelope_period = value & 0x07;

        // DAC 啟用條件：volume 或 direction 非零
        self.dac_enabled = (value & 0xF8) != 0;
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    fn write_nr13(&mut self, value: u8) {
        self.frequency = (self.frequency & 0x0700) | value as u16;
    }

    fn write_nr14(&mut self, value: u8, frame_step: u8) {
        self.frequency = (self.frequency & 0x00FF) | ((value as u16 & 0x07) << 8);

        let prev_length_enabled = self.length_enabled;
        self.length_enabled = (value & 0x40) != 0;

        // 額外長度時鐘（frame sequencer 怪癖）
        // 在第一個半週期（下一步會時鐘長度）啟用長度時，會額外減少一次
        if !prev_length_enabled
            && self.length_enabled
            && self.length_counter > 0
            && (frame_step & 1) == 0
        {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }

        if (value & 0x80) != 0 {
            self.trigger(frame_step);
        }
    }

    fn trigger(&mut self, frame_step: u8) {
        self.enabled = self.dac_enabled;
        self.frequency_timer = (2048 - self.frequency) * 4;

        // 重新載入長度計數器
        if self.length_counter == 0 {
            self.length_counter = 64;
            // 在第一個半週期觸發且啟用長度，會額外減少
            if self.length_enabled && (frame_step & 1) == 0 {
                self.length_counter -= 1;
            }
        }

        // 重新載入包絡
        self.envelope_timer = self.envelope_period;
        self.current_volume = self.envelope_volume;

        // 掃描初始化
        if self.has_sweep {
            self.sweep_shadow = self.frequency;
            self.sweep_timer = if self.sweep_period == 0 {
                8
            } else {
                self.sweep_period
            };
            self.sweep_enabled = self.sweep_period != 0 || self.sweep_shift != 0;
            self.sweep_negate_used = false;

            if self.sweep_shift != 0 {
                let _ = self.calculate_sweep_frequency();
            }
        }
    }

    fn tick(&mut self) {
        if self.frequency_timer > 0 {
            self.frequency_timer -= 1;
        }

        if self.frequency_timer == 0 {
            self.frequency_timer = (2048 - self.frequency) * 4;
            self.duty_position = (self.duty_position + 1) & 7;
        }
    }

    fn clock_length(&mut self) {
        if self.length_enabled && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    fn clock_envelope(&mut self) {
        if self.envelope_timer > 0 {
            self.envelope_timer -= 1;
        }

        if self.envelope_timer == 0 {
            self.envelope_timer = if self.envelope_period == 0 {
                8
            } else {
                self.envelope_period
            };

            if self.envelope_period != 0 {
                if self.envelope_direction && self.current_volume < 15 {
                    self.current_volume += 1;
                } else if !self.envelope_direction && self.current_volume > 0 {
                    self.current_volume -= 1;
                }
            }
        }
    }

    fn clock_sweep(&mut self) {
        if !self.has_sweep {
            return;
        }

        if self.sweep_timer > 0 {
            self.sweep_timer -= 1;
        }

        if self.sweep_timer == 0 {
            self.sweep_timer = if self.sweep_period == 0 {
                8
            } else {
                self.sweep_period
            };

            if self.sweep_enabled && self.sweep_period != 0 {
                let new_freq = self.calculate_sweep_frequency();

                if new_freq <= 2047 && self.sweep_shift != 0 {
                    self.frequency = new_freq;
                    self.sweep_shadow = new_freq;

                    // 再次計算以檢查溢出
                    let _ = self.calculate_sweep_frequency();
                }
            }
        }
    }

    fn calculate_sweep_frequency(&mut self) -> u16 {
        let delta = self.sweep_shadow >> self.sweep_shift;
        let new_freq = if self.sweep_negate {
            self.sweep_negate_used = true;
            self.sweep_shadow.wrapping_sub(delta)
        } else {
            self.sweep_shadow.wrapping_add(delta)
        };

        if new_freq > 2047 {
            self.enabled = false;
        }

        new_freq
    }

    fn get_output(&self) -> f32 {
        if !self.enabled || !self.dac_enabled {
            return 0.0;
        }

        let duty_patterns: [[u8; 8]; 4] = [
            [0, 0, 0, 0, 0, 0, 0, 1], // 12.5%
            [1, 0, 0, 0, 0, 0, 0, 1], // 25%
            [1, 0, 0, 0, 0, 1, 1, 1], // 50%
            [0, 1, 1, 1, 1, 1, 1, 0], // 75%
        ];

        let sample = duty_patterns[self.duty as usize][self.duty_position as usize];
        let dac_input = if sample != 0 { self.current_volume } else { 0 };

        // DAC 輸出：將 0-15 映射到 -1.0 到 1.0
        (dac_input as f32 / 7.5) - 1.0
    }
}

// 波形通道
struct WaveChannel {
    enabled: bool,
    dac_enabled: bool,

    frequency: u16,
    frequency_timer: u16,

    length_counter: u16,
    length_enabled: bool,

    volume_code: u8,
    position: u8,

    wave_ram: [u8; 16],
    sample_buffer: u8,

    // 用於追蹤 Wave RAM 讀取時機
    just_accessed_wave_ram: bool,
    last_wave_ram_byte: u8,

    nr30: u8,
    nr32: u8,
}

impl WaveChannel {
    fn new() -> Self {
        WaveChannel {
            enabled: false,
            dac_enabled: false,
            frequency: 0,
            frequency_timer: 0,
            length_counter: 0,
            length_enabled: false,
            volume_code: 0,
            position: 0,
            wave_ram: [0; 16],
            sample_buffer: 0,
            just_accessed_wave_ram: false,
            last_wave_ram_byte: 0,
            nr30: 0,
            nr32: 0,
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn power_off(&mut self) {
        self.enabled = false;
        self.dac_enabled = false;
        self.frequency = 0;
        self.length_enabled = false;
        self.volume_code = 0;
        self.nr30 = 0;
        self.nr32 = 0;
    }

    fn read_nr30(&self) -> u8 {
        self.nr30
    }

    fn read_nr32(&self) -> u8 {
        self.nr32
    }

    fn read_nr34(&self) -> u8 {
        if self.length_enabled { 0x40 } else { 0x00 }
    }

    fn write_nr30(&mut self, value: u8) {
        self.nr30 = value;
        self.dac_enabled = (value & 0x80) != 0;
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    fn write_nr31(&mut self, value: u8) {
        self.length_counter = 256 - value as u16;
    }

    fn write_nr32(&mut self, value: u8) {
        self.nr32 = value;
        self.volume_code = (value >> 5) & 0x03;
    }

    fn write_nr33(&mut self, value: u8) {
        self.frequency = (self.frequency & 0x0700) | value as u16;
    }

    fn write_nr34(&mut self, value: u8, frame_step: u8) {
        self.frequency = (self.frequency & 0x00FF) | ((value as u16 & 0x07) << 8);

        let prev_length_enabled = self.length_enabled;
        self.length_enabled = (value & 0x40) != 0;

        if !prev_length_enabled
            && self.length_enabled
            && self.length_counter > 0
            && (frame_step & 1) == 0
        {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }

        if (value & 0x80) != 0 {
            self.trigger(frame_step);
        }
    }

    fn trigger(&mut self, frame_step: u8) {
        // DMG Wave 通道 corruption bug:
        // 當 Wave 通道正在播放時被重新觸發，Wave RAM 可能會損壞
        // 這個 bug 需要非常精確的 T-cycle 級別時序才能正確實現
        // 許多模擬器選擇不實現這個 bug，因為它很少影響實際遊戲
        // 只有 Duck Tales 等少數遊戲會觸發這個問題

        // 波形通道觸發時重新啟用
        self.enabled = self.dac_enabled;

        // 頻率計時器重新載入（有額外延遲）
        self.frequency_timer = (2048 - self.frequency) * 2 + 6;

        if self.length_counter == 0 {
            self.length_counter = 256;
            if self.length_enabled && (frame_step & 1) == 0 {
                self.length_counter -= 1;
            }
        }

        // 位置重置為 0
        self.position = 0;
        // 重新讀取第一個樣本
        self.sample_buffer = self.wave_ram[0] >> 4;
    }

    fn tick(&mut self) {
        // 清除 Wave RAM 存取標記
        self.just_accessed_wave_ram = false;

        if self.frequency_timer > 0 {
            self.frequency_timer -= 1;
        }

        if self.frequency_timer == 0 {
            self.frequency_timer = (2048 - self.frequency) * 2;
            self.position = (self.position + 1) & 31;

            let byte_index = (self.position / 2) as usize;
            self.last_wave_ram_byte = self.wave_ram[byte_index];
            self.just_accessed_wave_ram = true;

            let nibble = if self.position & 1 == 0 {
                self.last_wave_ram_byte >> 4
            } else {
                self.last_wave_ram_byte & 0x0F
            };
            self.sample_buffer = nibble;
        }
    }

    fn clock_length(&mut self) {
        if self.length_enabled && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    fn read_wave_ram(&self, offset: u16) -> u8 {
        if self.enabled {
            // DMG: 當通道啟用時，只有在 APU 剛存取 Wave RAM 的短暫窗口內
            // 才能讀到正確的值，否則返回 0xFF
            if self.just_accessed_wave_ram {
                self.last_wave_ram_byte
            } else {
                0xFF
            }
        } else {
            self.wave_ram[offset as usize]
        }
    }

    fn write_wave_ram(&mut self, offset: u16, value: u8) {
        if self.enabled {
            // DMG: 當通道啟用時，只有在 APU 剛存取 Wave RAM 的短暫窗口內
            // 才能寫入，寫入的位置是當前 position 對應的字節
            if self.just_accessed_wave_ram {
                self.wave_ram[(self.position / 2) as usize] = value;
            }
            // 其他時候寫入被忽略
        } else {
            self.wave_ram[offset as usize] = value;
        }
    }

    fn get_output(&self) -> f32 {
        if !self.enabled || !self.dac_enabled {
            return 0.0;
        }

        let volume_shift = match self.volume_code {
            0 => 4, // 靜音
            1 => 0, // 100%
            2 => 1, // 50%
            3 => 2, // 25%
            _ => 4,
        };

        let dac_input = self.sample_buffer >> volume_shift;
        (dac_input as f32 / 7.5) - 1.0
    }
}

// 噪音通道
struct NoiseChannel {
    enabled: bool,
    dac_enabled: bool,

    // 頻率
    divisor_code: u8,
    clock_shift: u8,
    width_mode: bool, // true = 7-bit, false = 15-bit
    frequency_timer: u32,

    // 長度
    length_counter: u16,
    length_enabled: bool,

    // 包絡
    envelope_volume: u8,
    envelope_direction: bool,
    envelope_period: u8,
    envelope_timer: u8,
    current_volume: u8,

    // LFSR
    lfsr: u16,

    nr42: u8,
    nr43: u8,
}

impl NoiseChannel {
    fn new() -> Self {
        NoiseChannel {
            enabled: false,
            dac_enabled: false,
            divisor_code: 0,
            clock_shift: 0,
            width_mode: false,
            frequency_timer: 0,
            length_counter: 0,
            length_enabled: false,
            envelope_volume: 0,
            envelope_direction: false,
            envelope_period: 0,
            envelope_timer: 0,
            current_volume: 0,
            lfsr: 0x7FFF,
            nr42: 0,
            nr43: 0,
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn power_off(&mut self) {
        self.enabled = false;
        self.dac_enabled = false;
        self.divisor_code = 0;
        self.clock_shift = 0;
        self.width_mode = false;
        self.length_enabled = false;
        self.envelope_volume = 0;
        self.envelope_direction = false;
        self.envelope_period = 0;
        self.current_volume = 0;
        self.nr42 = 0;
        self.nr43 = 0;
    }

    fn read_nr42(&self) -> u8 {
        self.nr42
    }

    fn read_nr43(&self) -> u8 {
        self.nr43
    }

    fn read_nr44(&self) -> u8 {
        if self.length_enabled { 0x40 } else { 0x00 }
    }

    fn write_nr41(&mut self, value: u8) {
        self.length_counter = 64 - (value & 0x3F) as u16;
    }

    fn write_nr42(&mut self, value: u8) {
        self.nr42 = value;
        self.envelope_volume = value >> 4;
        self.envelope_direction = (value & 0x08) != 0;
        self.envelope_period = value & 0x07;

        self.dac_enabled = (value & 0xF8) != 0;
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    fn write_nr43(&mut self, value: u8) {
        self.nr43 = value;
        self.clock_shift = value >> 4;
        self.width_mode = (value & 0x08) != 0;
        self.divisor_code = value & 0x07;
    }

    fn write_nr44(&mut self, value: u8, frame_step: u8) {
        let prev_length_enabled = self.length_enabled;
        self.length_enabled = (value & 0x40) != 0;

        if !prev_length_enabled
            && self.length_enabled
            && self.length_counter > 0
            && (frame_step & 1) == 0
        {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }

        if (value & 0x80) != 0 {
            self.trigger(frame_step);
        }
    }

    fn trigger(&mut self, frame_step: u8) {
        self.enabled = self.dac_enabled;
        self.frequency_timer = self.get_frequency_timer();

        if self.length_counter == 0 {
            self.length_counter = 64;
            if self.length_enabled && (frame_step & 1) == 0 {
                self.length_counter -= 1;
            }
        }

        self.envelope_timer = self.envelope_period;
        self.current_volume = self.envelope_volume;

        self.lfsr = 0x7FFF;
    }

    fn get_frequency_timer(&self) -> u32 {
        let divisor: u32 = match self.divisor_code {
            0 => 8,
            n => (n as u32) << 4,
        };
        divisor << self.clock_shift
    }

    fn tick(&mut self) {
        if self.frequency_timer > 0 {
            self.frequency_timer -= 1;
        }

        if self.frequency_timer == 0 {
            self.frequency_timer = self.get_frequency_timer();

            // LFSR 時鐘
            let xor_result = (self.lfsr & 1) ^ ((self.lfsr >> 1) & 1);
            self.lfsr = (self.lfsr >> 1) | (xor_result << 14);

            if self.width_mode {
                // 7-bit 模式
                self.lfsr &= !(1 << 6);
                self.lfsr |= xor_result << 6;
            }
        }
    }

    fn clock_length(&mut self) {
        if self.length_enabled && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    fn clock_envelope(&mut self) {
        if self.envelope_timer > 0 {
            self.envelope_timer -= 1;
        }

        if self.envelope_timer == 0 {
            self.envelope_timer = if self.envelope_period == 0 {
                8
            } else {
                self.envelope_period
            };

            if self.envelope_period != 0 {
                if self.envelope_direction && self.current_volume < 15 {
                    self.current_volume += 1;
                } else if !self.envelope_direction && self.current_volume > 0 {
                    self.current_volume -= 1;
                }
            }
        }
    }

    fn get_output(&self) -> f32 {
        if !self.enabled || !self.dac_enabled {
            return 0.0;
        }

        // LFSR bit 0 為 0 時輸出音量，為 1 時輸出 0
        let dac_input = if (self.lfsr & 1) == 0 {
            self.current_volume
        } else {
            0
        };

        (dac_input as f32 / 7.5) - 1.0
    }
}
