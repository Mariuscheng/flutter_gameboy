// Game Boy 模擬器主結構

use crate::apu::Apu;
use crate::cpu::Cpu;
use crate::joypad::Joypad;
use crate::mmu::{IoHandler, Mmu};
use crate::ppu::Ppu;
use crate::timer::Timer;
use std::time::Instant;

#[cfg(debug_assertions)]
fn log_ff00_read(value: u8, select: u8, action_keys: u8, direction_keys: u8) {
    let _ = (value, select, action_keys, direction_keys);
}

#[cfg(not(debug_assertions))]
fn log_ff00_read(_value: u8, _select: u8, _action_keys: u8, _direction_keys: u8) {}

#[cfg(debug_assertions)]
fn log_ff00_write(value: u8, result: u8, select: u8, action_keys: u8, direction_keys: u8) {
    let _ = (value, result, select, action_keys, direction_keys);
}

#[cfg(not(debug_assertions))]
fn log_ff00_write(_value: u8, _result: u8, _select: u8, _action_keys: u8, _direction_keys: u8) {}

#[cfg(debug_assertions)]
pub(crate) fn log_joypad_interrupt_service(pc: u16, ie: u8, iff: u8) {
    let _ = (pc, ie, iff);
}

#[cfg(not(debug_assertions))]
pub(crate) fn log_joypad_interrupt_service(_pc: u16, _ie: u8, _iff: u8) {}

// Custom error types for better error handling (Rust 1.93.0 improvements)
#[derive(Debug)]
#[allow(dead_code)]
pub enum GameBoyError {
    RomLoad {
        path: String,
        source: Box<dyn std::error::Error>,
    },
    Timing(String),
    Interrupt(String),
    Io(std::io::Error),
}

impl std::fmt::Display for GameBoyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameBoyError::RomLoad { path, source } => {
                write!(f, "Failed to load ROM '{}': {}", path, source)
            }
            GameBoyError::Timing(msg) => write!(f, "Timing error: {}", msg),
            GameBoyError::Interrupt(msg) => write!(f, "Interrupt error: {}", msg),
            GameBoyError::Io(err) => write!(f, "I/O error: {}", err),
        }
    }
}

impl std::error::Error for GameBoyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GameBoyError::RomLoad { source, .. } => Some(source.as_ref()),
            GameBoyError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for GameBoyError {
    fn from(err: std::io::Error) -> Self {
        GameBoyError::Io(err)
    }
}

/// Game Boy interrupt types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InterruptType {
    VBlank,
    LcdStat,
    Timer,
    Serial,
    Joypad,
}

/// Joypad interrupt delay tracking
struct JoypadInterruptDelay {
    cycles_remaining: u8,
}

/// Interrupt handler
pub struct InterruptHandler {
    pub ie_register: u8,
    pub if_register: u8,
    joypad_interrupt_delay: Option<JoypadInterruptDelay>,
}

struct GameBoyIoWrapper {
    ppu: *mut Ppu,
    apu: *mut Apu,
    timer: *mut Timer,
    joypad: *mut Joypad,
    interrupt_handler: *mut InterruptHandler,
}

impl GameBoyIoWrapper {
    fn new(
        ppu: *mut Ppu,
        apu: *mut Apu,
        timer: *mut Timer,
        joypad: *mut Joypad,
        interrupt_handler: *mut InterruptHandler,
    ) -> Self {
        GameBoyIoWrapper {
            ppu,
            apu,
            timer,
            joypad,
            interrupt_handler,
        }
    }
}

impl IoHandler for GameBoyIoWrapper {
    fn read_io(&self, address: u16) -> u8 {
        unsafe {
            match address {
                0xFF00 => {
                    if !self.joypad.is_null() {
                        let joypad = &*self.joypad;
                        let value = joypad.read_register();
                        log_ff00_read(
                            value,
                            joypad.select,
                            joypad.action_keys,
                            joypad.direction_keys,
                        );
                        value
                    } else {
                        0xFF
                    }
                }
                0xFF04..=0xFF07 => {
                    if !self.timer.is_null() {
                        (*self.timer).read_register(address)
                    } else {
                        0
                    }
                }
                0xFF10..=0xFF3F => {
                    if !self.apu.is_null() {
                        (*self.apu).read_register(address)
                    } else {
                        0
                    }
                }
                0xFF40..=0xFF4B => {
                    if !self.ppu.is_null() {
                        (*self.ppu).read_register(address)
                    } else {
                        0
                    }
                }
                0xFF0F => {
                    if !self.interrupt_handler.is_null() {
                        (*self.interrupt_handler).if_register | 0xE0
                    } else {
                        0xE0
                    }
                }
                0xFFFF => {
                    if !self.interrupt_handler.is_null() {
                        (*self.interrupt_handler).ie_register
                    } else {
                        0
                    }
                }
                _ => 0,
            }
        }
    }

    fn write_io(&mut self, address: u16, value: u8, interrupt_flags: &mut u8) {
        unsafe {
            match address {
                0xFF00 => {
                    if !self.joypad.is_null() {
                        let joypad = self.joypad as *mut Joypad;
                        (*joypad).write_register(value);
                        log_ff00_write(
                            value,
                            (*joypad).read_register(),
                            (*joypad).select,
                            (*joypad).action_keys,
                            (*joypad).direction_keys,
                        );
                    }
                }
                0xFF04..=0xFF07 => {
                    if !self.timer.is_null() {
                        let timer = self.timer as *mut Timer;
                        (*timer).write_register(address, value, interrupt_flags);
                    }
                }
                0xFF10..=0xFF3F => {
                    if !self.apu.is_null() {
                        let apu = self.apu as *mut Apu;
                        (*apu).write_register(address, value);
                    }
                }
                0xFF40..=0xFF4B => {
                    if !self.ppu.is_null() {
                        let ppu = self.ppu as *mut Ppu;
                        (*ppu).write_register(address, value, interrupt_flags);
                    }
                }
                0xFF0F => {
                    if !self.interrupt_handler.is_null() {
                        let handler = self.interrupt_handler as *mut InterruptHandler;
                        (*handler).if_register = value | 0xE0;
                    }
                }
                0xFFFF => {
                    if !self.interrupt_handler.is_null() {
                        let handler = self.interrupt_handler as *mut InterruptHandler;
                        (*handler).ie_register = value;
                    }
                }
                _ => {}
            }
        }
    }
}

pub struct GameBoy {
    pub cpu: Cpu,
    pub mmu: Mmu,
    pub ppu: Ppu,
    pub apu: Apu,
    pub timer: Timer,
    pub joypad: Joypad,
    pub interrupt_handler: InterruptHandler,
    #[allow(dead_code)]
    pub cycles: u64,
}

impl GameBoy {
    pub fn new() -> Box<Self> {
        let mut gb = Box::new(GameBoy {
            cpu: Cpu::new(),
            mmu: Mmu::new(),
            ppu: Ppu::new(),
            apu: Apu::new(),
            timer: Timer::new(),
            joypad: Joypad::new(),
            interrupt_handler: InterruptHandler::new(),
            cycles: 0,
        });

        // 設置 I/O 處理器
        let ppu_ptr = std::ptr::addr_of_mut!(gb.ppu);
        let apu_ptr = std::ptr::addr_of_mut!(gb.apu);
        let timer_ptr = std::ptr::addr_of_mut!(gb.timer);
        let joypad_ptr = std::ptr::addr_of_mut!(gb.joypad);
        let interrupt_handler_ptr = std::ptr::addr_of_mut!(gb.interrupt_handler);

        let io_wrapper = GameBoyIoWrapper::new(
            ppu_ptr,
            apu_ptr,
            timer_ptr,
            joypad_ptr,
            interrupt_handler_ptr,
        );
        gb.mmu.set_io_handler(Box::new(io_wrapper));

        // 讓 MMU 能依 PPU mode 套用 VRAM/OAM CPU 存取限制
        gb.mmu.set_ppu(ppu_ptr);

        // 設置組件間的引用
        gb.joypad.set_interrupt_handler(interrupt_handler_ptr);

        // 設置初始硬體狀態 (模擬啟動後狀態)
        gb.mmu.write_byte(0xFFFF, 0x00); // 關閉所有中斷
        gb.mmu.write_byte(0xFF40, 0x91); // 啟用 LCD, 背景, 圖塊集 0
        gb.mmu.write_byte(0xFF41, 0x85); // STAT
        gb.mmu.write_byte(0xFF44, 0x00); // LY

        gb
    }

    // 載入 ROM
    pub fn load_rom(&mut self, path: &str) -> Result<(), GameBoyError> {
        self.mmu
            .load_rom_from_bytes(std::fs::read(path).unwrap())
            .map_err(|e| GameBoyError::RomLoad {
                path: path.to_string(),
                source: e,
            })?;

        self.interrupt_handler.auto_configure_for_game(
            std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(""),
        );
        Ok(())
    }

    pub fn load_rom_from_bytes(&mut self, bytes: Vec<u8>) -> Result<(), GameBoyError> {
        self.mmu
            .load_rom_from_bytes(bytes)
            .map_err(|e| GameBoyError::RomLoad {
                path: "Memory Buffer".to_string(),
                source: e,
            })?;

        self.interrupt_handler
            .auto_configure_for_game("Memory Emulator");
        Ok(())
    }

    pub fn step_frame(&mut self) {
        self.run_frame();
    }

    pub fn run_instructions(&mut self, instructions_count: u32) {
        for _ in 0..instructions_count {
            let instruction_cycles = self.step_cpu_with_timing();
            self.cycles = self.cycles.saturating_add(instruction_cycles as u64);
        }
    }

    // 執行到一幀完成（進入 VBlank）為止，確保畫面呈現穩定
    pub fn run_frame(&mut self) {
        // 清掉上一幀可能殘留的 ready
        let _ = self.ppu.take_frame_ready();

        // 以「進入 VBlank 的升緣」作為幀完成訊號：不會被指令邊界漏掉
        loop {
            let _ = self.step_cpu_with_timing();

            if self.ppu.take_frame_ready() {
                break;
            }
        }
    }

    pub fn get_present_framebuffer(&self) -> &[u8] {
        self.ppu.get_present_framebuffer()
    }

    fn consume_external_interrupts(&mut self) {
        let external_flags = self.interrupt_handler.if_register;
        if external_flags != 0 {
            let merged_if = self.mmu.read_byte(0xFF0F) | external_flags | 0xE0;
            self.mmu.write_byte(0xFF0F, merged_if);
            self.mmu.if_reg = merged_if;
            self.interrupt_handler.if_register = 0;
        }
    }

    // 執行一個 CPU 指令，並在執行期間同步更新 Timer 和 PPU
    fn step_cpu_with_timing(&mut self) -> u32 {
        // 1. 將按鍵等外部觸發的中斷標記匯入 MMU
        self.consume_external_interrupts();

        // 2. 執行一次 CPU 指令
        let cycles = self.cpu.step(&mut self.mmu);

        // 3. 取得執行後的 IF
        let mut if_reg = self.mmu.read_byte(0xFF0F);

        // 4. 更新組件
        for _cycle in 0..cycles {
            self.ppu.tick(&self.mmu, &mut if_reg);
            self.timer.tick(&mut if_reg);
            self.apu.tick();
        }

        // 5. 寫回 MMU
        self.mmu.write_byte(0xFF0F, if_reg);
        self.mmu.if_reg = if_reg;

        cycles
    }

    // 獲取當前畫面緩衝區
    #[allow(dead_code)]
    pub fn get_framebuffer(&self) -> &[u8] {
        self.ppu.get_present_framebuffer()
    }
}

impl InterruptHandler {
    pub fn new() -> Self {
        Self {
            ie_register: 0,
            if_register: 0xE0,
            joypad_interrupt_delay: None,
        }
    }

    pub fn has_pending_interrupts(&self) -> bool {
        (self.ie_register & self.if_register & 0x1F) != 0
    }

    pub fn get_highest_priority_interrupt(&self) -> Option<(InterruptType, u16)> {
        let pending = self.ie_register & self.if_register & 0x1F;
        if pending == 0 {
            return None;
        }

        // Game Boy 中斷優先級：VBlank > LCD > Timer > Serial > Joypad
        for bit in 0..5 {
            if (pending & (1 << bit)) != 0 {
                let interrupt_type = match bit {
                    0 => InterruptType::VBlank,
                    1 => InterruptType::LcdStat,
                    2 => InterruptType::Timer,
                    3 => InterruptType::Serial,
                    4 => InterruptType::Joypad,
                    _ => continue,
                };
                let vector = match bit {
                    0 => 0x40,
                    1 => 0x48,
                    2 => 0x50,
                    3 => 0x58,
                    4 => 0x60,
                    _ => 0,
                };
                return Some((interrupt_type, vector));
            }
        }
        None
    }

    pub fn trigger_interrupt(&mut self, interrupt_type: InterruptType) {
        let bit = match interrupt_type {
            InterruptType::VBlank => 0,
            InterruptType::LcdStat => 1,
            InterruptType::Timer => 2,
            InterruptType::Serial => 3,
            InterruptType::Joypad => 4,
        };

        self.if_register = (self.if_register | (1 << bit)) | 0xE0;
    }

    pub fn process_joypad_interrupt_delay(&mut self) -> bool {
        if let Some(ref mut delay) = self.joypad_interrupt_delay {
            delay.cycles_remaining = delay.cycles_remaining.saturating_sub(1);
            if delay.cycles_remaining == 0 {
                self.if_register = (self.if_register | 0x10) | 0xE0;
                self.joypad_interrupt_delay = None;
                return true;
            }
        }
        false
    }

    pub fn clear_interrupt_flag(&mut self, interrupt_type: InterruptType) {
        let bit = match interrupt_type {
            InterruptType::VBlank => 0,
            InterruptType::LcdStat => 1,
            InterruptType::Timer => 2,
            InterruptType::Serial => 3,
            InterruptType::Joypad => 4,
        };
        self.if_register = (self.if_register & !(1 << bit)) | 0xE0;
    }

    pub fn acknowledge_interrupt(&mut self, interrupt_type: InterruptType, _start_time: Instant) {
        self.clear_interrupt_flag(interrupt_type);
    }

    pub fn auto_configure_for_game(&mut self, _rom_filename: &str) {}
}

impl Default for InterruptHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::GameBoy;
    use crate::cpu::{CpuState, InterruptMasterState};
    use crate::joypad::JoypadKey;

    #[test]
    fn joypad_interrupt_is_forwarded_into_mmu_if_register() {
        let mut gameboy = GameBoy::new();

        gameboy.joypad.write_register(0x20);
        gameboy.joypad.set_key(JoypadKey::Right, true);

        assert_eq!(gameboy.interrupt_handler.if_register & 0x10, 0x10);

        gameboy.consume_external_interrupts();

        assert_eq!(gameboy.mmu.read_byte(0xFF0F) & 0x10, 0x10);
        assert_eq!(gameboy.interrupt_handler.if_register & 0x1F, 0x00);
    }

    #[test]
    fn cpu_reads_selected_ff00_row_from_mmu() {
        let mut gameboy = GameBoy::new();

        gameboy.mmu.write_byte(0xFF00, 0x20);
        gameboy.joypad.set_key(JoypadKey::Right, true);

        assert_eq!(gameboy.mmu.read_byte(0xFF00) & 0x0F, 0x0E);

        gameboy.mmu.write_byte(0xFF00, 0x10);
        gameboy.joypad.set_key(JoypadKey::Start, true);

        assert_eq!(gameboy.mmu.read_byte(0xFF00) & 0x0F, 0x07);
    }

    #[test]
    fn joypad_interrupt_wakes_halted_cpu() {
        let mut gameboy = GameBoy::new();

        gameboy.cpu.state = CpuState::Halted;
        gameboy.cpu.ime = InterruptMasterState::Enabled;
        gameboy.mmu.write_byte(0xFFFF, 0x10);
        gameboy.mmu.write_byte(0xFF00, 0x20);
        gameboy.joypad.set_key(JoypadKey::Right, true);

        let cycles = gameboy.step_cpu_with_timing();

        assert_eq!(cycles, 20);
        assert_eq!(gameboy.cpu.state, CpuState::Running);
        assert_eq!(gameboy.cpu.pc, 0x60);
    }
}
