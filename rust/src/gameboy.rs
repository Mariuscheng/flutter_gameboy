// Game Boy 模擬器主結構

use crate::apu::Apu;
use crate::cpu::{Cpu, CpuState, InterruptMasterState, OPCODES, Opcode};
use crate::joypad::Joypad;
use crate::mmu::{IoHandler, Mmu};
use crate::ppu::Ppu;
use crate::timer::Timer;
use std::time::Instant;

/// 這條指令會做幾次資料匯流排存取（不含程式計數器與堆疊）。
///
/// 讀-改-寫指令（INC/DEC (HL)、CB 的移位/旋轉/RES/SET (HL)）會先讀再寫，
/// 其餘記憶體指令只有一次存取。硬體上單一存取在最後一個 cycle，
/// 讀-改-寫則是倒數第二個 cycle 讀、最後一個 cycle 寫。
fn bus_accesses(opcode: &Opcode) -> u32 {
    let has_memory_operand = opcode
        .operands
        .iter()
        .any(|operand| operand.immediate == Some(false));
    if !has_memory_operand {
        return 0;
    }

    let is_read_modify_write = matches!(
        opcode.mnemonic.as_str(),
        "INC"
            | "DEC"
            | "RLC"
            | "RRC"
            | "RL"
            | "RR"
            | "SLA"
            | "SRA"
            | "SRL"
            | "SWAP"
            | "RES"
            | "SET"
    );

    if is_read_modify_write { 2 } else { 1 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OamBusEvent {
    Read(u16),
    Write(u16),
}

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
#[derive(Debug)]
struct JoypadInterruptDelay {
    cycles_remaining: u8,
}

/// Interrupt handler
#[derive(Debug)]
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

/// 只有當子系統指標已接上時才執行 `f`，把「可為空」的判斷集中在這裡。
unsafe fn write_if_present<T, F: FnOnce(&mut T)>(pointer: *mut T, f: F) {
    // SAFETY: 呼叫端保證 `pointer` 要嘛是 null、要嘛指向仍存活的 `T`。
    if let Some(target) = unsafe { pointer.as_mut() } {
        f(target);
    }
}

impl IoHandler for GameBoyIoWrapper {
    fn read_io(&self, address: u16) -> u8 {
        // 子系統以 `Option` 表示「是否已接上」，避免每個分支重複 `is_null` 判斷。
        unsafe {
            match address {
                0xFF00 => self.joypad.as_ref().map_or(0xFF, |joypad| {
                    let value = joypad.read_register();
                    log_ff00_read(
                        value,
                        joypad.select,
                        joypad.action_keys,
                        joypad.direction_keys,
                    );
                    value
                }),
                0xFF04..=0xFF07 => self
                    .timer
                    .as_ref()
                    .map_or(0, |timer| timer.read_register(address)),
                0xFF10..=0xFF3F => self
                    .apu
                    .as_ref()
                    .map_or(0, |apu| apu.read_register(address)),
                0xFF40..=0xFF4B => self
                    .ppu
                    .as_ref()
                    .map_or(0, |ppu| ppu.read_register(address)),
                0xFF0F => self
                    .interrupt_handler
                    .as_ref()
                    .map_or(0xE0, |handler| handler.if_register | 0xE0),
                0xFFFF => self
                    .interrupt_handler
                    .as_ref()
                    .map_or(0, |handler| handler.ie_register),
                _ => 0,
            }
        }
    }

    fn write_io(&mut self, address: u16, value: u8, interrupt_flags: &mut u8) {
        unsafe {
            match address {
                0xFF00 => write_if_present(self.joypad, |joypad| {
                    joypad.write_register(value);
                    log_ff00_write(
                        value,
                        joypad.read_register(),
                        joypad.select,
                        joypad.action_keys,
                        joypad.direction_keys,
                    );
                }),
                0xFF04..=0xFF07 => write_if_present(self.timer, |timer| {
                    timer.write_register(address, value, interrupt_flags);
                }),
                0xFF10..=0xFF3F => write_if_present(self.apu, |apu| {
                    apu.write_register(address, value);
                }),
                0xFF40..=0xFF4B => write_if_present(self.ppu, |ppu| {
                    ppu.write_register(address, value, interrupt_flags);
                }),
                0xFF0F => write_if_present(self.interrupt_handler, |handler| {
                    handler.if_register = value | 0xE0;
                }),
                0xFFFF => write_if_present(self.interrupt_handler, |handler| {
                    handler.ie_register = value;
                }),
                _ => {}
            }
        }
    }
}

#[derive(Debug)]
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

        // 2. 16-bit INC/DEC 會在 IDU 內部把暫存器值直接放到位址匯流排；
        //    若值落在 OAM 範圍且 PPU 正在 mode 2，會觸發 OAM corruption bug。
        if let Some(value) = self.peek_inc_dec_rr()
            && (0xFE00..=0xFEFF).contains(&value)
        {
            let mut if_reg = self.mmu.read_byte(0xFF0F);
            let delay = 4u32;
            self.tick_components(delay, &mut if_reg);
            self.write_if(if_reg);

            if let Some(row) = self.ppu.oam_row {
                self.mmu.oam_write_corruption(row);
            }

            let cycles = self.cpu.step(&mut self.mmu);
            let mut if_reg = self.mmu.read_byte(0xFF0F);
            self.tick_components(cycles.saturating_sub(delay), &mut if_reg);
            self.write_if(if_reg);
            return cycles;
        }

        // 3. POP/PUSH/LD A,(HL±) 在指令內部有多個 OAM 匯流排事件，
        //    依硬體 cycle 逐一觸發（read/write corruption）。
        let oam_events = self.oam_events_for_next_instruction();
        if !oam_events.is_empty() {
            let mut if_reg = self.mmu.read_byte(0xFF0F);
            let mut consumed = 0u32;
            for (offset, event) in &oam_events {
                let target = (*offset).min(16);
                if target > consumed {
                    self.tick_components(target - consumed, &mut if_reg);
                    consumed = target;
                    self.write_if(if_reg);
                }

                if let Some(row) = self.ppu.oam_row {
                    match *event {
                        OamBusEvent::Read(address) if (0xFE00..=0xFEFF).contains(&address) => {
                            self.mmu.oam_read_corruption(row);
                        }
                        OamBusEvent::Write(address) if (0xFE00..=0xFEFF).contains(&address) => {
                            self.mmu.oam_write_corruption(row);
                        }
                        _ => {}
                    }
                }
            }

            let cycles = self.cpu.step(&mut self.mmu);
            let mut if_reg = self.mmu.read_byte(0xFF0F);
            self.tick_components(cycles.saturating_sub(consumed), &mut if_reg);
            self.write_if(if_reg);
            return cycles;
        }

        // 3. 先看這一條指令會在匯流排上做幾次資料存取。
        //    CPU 是一個指令執行完才一次讀寫，所以要在執行前先把週期補到
        //    存取發生的位置，讀寫才會落在硬體正確的 cycle 上。
        let Some((instruction_cycles, bus_accesses)) = self.instruction_bus_profile() else {
            // 沒有資料存取（或中斷/ halted）：維持原本「執行完整條指令後再一次補週期」。
            let cycles = self.cpu.step(&mut self.mmu);
            let mut if_reg = self.mmu.read_byte(0xFF0F);
            self.tick_components(cycles, &mut if_reg);
            self.write_if(if_reg);
            return cycles;
        };

        let mut if_reg = self.mmu.read_byte(0xFF0F);
        let mut consumed_cycles = 0;

        // 單一讀寫：存取在最後一個 cycle（最後一個 T-cycle），先補 cycles-1。
        // 讀-改-寫：讀在倒數第二個 cycle、寫在最後一個 cycle；兩者相差一個
        // M-cycle (4 T-cycles)，所以先補 cycles-5，讀完再隔 4 個 T-cycle 才寫入。
        let (pre_ticks, gap_ticks) = if bus_accesses == 2 {
            (instruction_cycles.saturating_sub(5), 4)
        } else {
            (instruction_cycles.saturating_sub(1), 1)
        };
        if pre_ticks > 0 {
            self.tick_components(pre_ticks, &mut if_reg);
            consumed_cycles += pre_ticks;
            // 讓 cpu.step() 看到補完週期後的中斷狀態
            self.write_if(if_reg);
        }

        if bus_accesses == 2 {
            self.mmu.capture_writes = true;
        }
        let cycles = self.cpu.step(&mut self.mmu);
        if bus_accesses == 2 {
            self.mmu.capture_writes = false;
        }

        let mut if_reg = self.mmu.read_byte(0xFF0F);

        if bus_accesses == 2 {
            if consumed_cycles < cycles {
                // 讀完之後、寫入之前隔一個 M-cycle：寫入落在最後一個 cycle
                let gap = gap_ticks.min(cycles - consumed_cycles);
                self.tick_components(gap, &mut if_reg);
                consumed_cycles += gap;
            }

            let writes = std::mem::take(&mut self.mmu.captured_writes);
            for (address, value) in writes {
                self.mmu.write_byte(address, value);
            }
        }

        let remaining = cycles.saturating_sub(consumed_cycles);
        self.tick_components(remaining, &mut if_reg);
        self.write_if(if_reg);

        cycles
    }

    // 補週期：PPU / Timer / APU 每個 T-cycle 各 tick 一次
    fn tick_components(&mut self, cycles: u32, if_reg: &mut u8) {
        for _cycle in 0..cycles {
            self.ppu.tick(&self.mmu, if_reg);
            self.timer.tick(if_reg);
            self.apu.tick();
        }
    }

    fn write_if(&mut self, if_reg: u8) {
        self.mmu.write_byte(0xFF0F, if_reg);
        self.mmu.if_reg = if_reg;
    }

    /// 下一條指令若是 16-bit INC/DEC（BC/DE/HL/SP），回傳操作前的暫存器值。
    fn peek_inc_dec_rr(&self) -> Option<u16> {
        if self.cpu.state == CpuState::Halted {
            return None;
        }
        if self.cpu.ime == InterruptMasterState::Enabled
            && (self.mmu.ie & self.mmu.if_reg & 0x1F) != 0
        {
            return None;
        }

        let opcode = self.mmu.read_byte_ppu(self.cpu.pc);
        match opcode {
            0x03 | 0x0B => Some(self.cpu.get_bc()),
            0x13 | 0x1B => Some(self.cpu.get_de()),
            0x23 | 0x2B => Some(self.cpu.get_hl()),
            0x33 | 0x3B => Some(self.cpu.sp),
            _ => None,
        }
    }

    /// POP/PUSH/LD A,(HL±) 這一類指令內部會產生多個 OAM 匯流排事件。
    /// 回傳 (T-cycle 偏移, 事件) 的清單，供呼叫端在正確時點觸發 corruption。
    fn oam_events_for_next_instruction(&self) -> Vec<(u32, OamBusEvent)> {
        if self.cpu.state == CpuState::Halted {
            return Vec::new();
        }
        if self.cpu.ime == InterruptMasterState::Enabled
            && (self.mmu.ie & self.mmu.if_reg & 0x1F) != 0
        {
            return Vec::new();
        }

        let mut events = Vec::new();
        let opcode = self.mmu.read_byte_ppu(self.cpu.pc);
        match opcode {
            // POP rr：M2/M3 各讀一次，讀取的位址也在 $FE00-$FEFF 時觸發
            // read corruption（POP 的兩次 SP 增量即表現在這兩個位址上）。
            0xC1 | 0xD1 | 0xE1 | 0xF1 => {
                let sp = self.cpu.sp;
                events.push((4, OamBusEvent::Read(sp)));
                events.push((8, OamBusEvent::Read(sp.wrapping_add(1))));
            }
            // PUSH rr：SP 的兩次遞減在 M2/M3/M4 三個 cycle 都會把值放上
            // 位址匯流排，各觸發一次 write corruption。觸發條件看的是遞減前
            // 的 SP（$FE00 仍會觸發，即使實際寫入位址已是 $FDFF）。
            0xC5 | 0xD5 | 0xE5 | 0xF5 => {
                let sp = self.cpu.sp;
                events.push((4, OamBusEvent::Write(sp)));
                events.push((8, OamBusEvent::Write(sp)));
                events.push((12, OamBusEvent::Write(sp)));
            }
            // LD A,(HL+) / LD A,(HL-)：M2 讀取時 HL 增量驅動位址匯流排
            0x2A | 0x3A => {
                let hl = self.cpu.get_hl();
                events.push((4, OamBusEvent::Read(hl)));
            }
            _ => {}
        }
        events
    }

    /// 回傳 (指令總 cycle 數, 資料匯流排存取次數)。
    ///
    /// 只處理「會在最後兩個 cycle 存取記憶體」的指令；中斷分派、HALT 與
    /// push/pop/call/ret 等堆疊指令不在這裡處理（堆疊存取不列入 mem_timing）。
    fn instruction_bus_profile(&self) -> Option<(u32, u32)> {
        if self.cpu.state == CpuState::Halted {
            return None;
        }
        if self.cpu.ime == InterruptMasterState::Enabled
            && (self.mmu.ie & self.mmu.if_reg & 0x1F) != 0
        {
            // 這一條會變成中斷分派，不要預先補週期
            return None;
        }

        let pc = self.cpu.pc;
        let first = self.mmu.read_byte_ppu(pc);
        let opcode = if first == 0xCB {
            let second = self.mmu.read_byte_ppu(pc.wrapping_add(1));
            OPCODES.cbprefixed[second as usize].as_ref()?
        } else {
            OPCODES.unprefixed[first as usize].as_ref()?
        };

        let accesses = bus_accesses(opcode);
        if accesses == 0 {
            return None;
        }

        let cycles = u32::from(*opcode.cycles.first()?);
        Some((cycles, accesses))
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
