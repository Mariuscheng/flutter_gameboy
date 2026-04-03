// Joypad (按鍵輸入) - 處理玩家輸入

use crate::gameboy::InterruptHandler;
use std::time::{Duration, Instant};

#[allow(missing_debug_implementations)]
pub struct Joypad {
    // 按鍵狀態 (0 代表按下，1 代表放開)
    // 位元: 0=A/右, 1=B/左, 2=Select/上, 3=Start/下
    pub action_keys: u8,
    pub direction_keys: u8,

    // 選取位元 (Bit 4: 方向鍵, Bit 5: 功能鍵)
    pub select: u8,

    // 精確狀態追蹤
    pub key_states: [KeyState; 8], // 8個按鍵的狀態
    pub debounce_filter: DebounceFilter,
    pub interrupt_handler: Option<*mut InterruptHandler>,
}

#[derive(Debug, Clone)]
pub struct KeyState {
    pub pressed: bool,
    pub last_change: Instant,
    pub press_duration: Duration,
    pub release_duration: Duration,
    pub bounce_count: u32, // 去抖動計數
}

impl Default for KeyState {
    fn default() -> Self {
        Self {
            pressed: false,
            last_change: Instant::now(),
            press_duration: Duration::ZERO,
            release_duration: Duration::ZERO,
            bounce_count: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DebounceFilter {
    pub debounce_threshold: Duration, // 去抖動閾值 (通常 5-10ms)
    pub bounce_events_filtered: u64,
}

impl Default for DebounceFilter {
    fn default() -> Self {
        Self {
            debounce_threshold: Duration::from_millis(5), // 5ms 去抖動
            bounce_events_filtered: 0,
        }
    }
}

impl Joypad {
    pub fn new() -> Self {
        let mut key_states = [
            KeyState::default(),
            KeyState::default(),
            KeyState::default(),
            KeyState::default(),
            KeyState::default(),
            KeyState::default(),
            KeyState::default(),
            KeyState::default(),
        ];

        // 初始化所有按鍵為放開狀態
        for state in &mut key_states {
            state.pressed = false;
            state.last_change = Instant::now() - Duration::from_secs(1); // 設置為過去的時間，以便第一次按鍵被接受
        }

        Joypad {
            action_keys: 0x0F,    // 預設為全放開 (1)
            direction_keys: 0x0F, // 預設為全放開 (1)
            select: 0x30,         // 預設為不選取 (11)
            key_states,
            debounce_filter: DebounceFilter::default(),
            interrupt_handler: None,
        }
    }

    /// 設置中斷處理器以進行優化的中斷處理
    pub fn set_interrupt_handler(&mut self, handler: *mut InterruptHandler) {
        self.interrupt_handler = Some(handler);
    }

    /// 輔助函數：更新位元狀態
    fn update_key_bit(target: &mut u8, mask: u8, pressed: bool) {
        if pressed {
            *target &= !mask;
        } else {
            *target |= mask;
        }
    }

    pub fn read_register(&self) -> u8 {
        // 高位元(6-7)讀取時通常為 1，位元 4-5 是 select bits
        let upper = 0xC0 | self.select;

        // 低 4 位預設為 1（沒有按鍵按下）
        let mut keys = 0x0F;

        if (self.select & 0x10) == 0 {
            // 已選取方向鍵 (Bit 4 = 0)
            keys &= self.direction_keys;
        }

        if (self.select & 0x20) == 0 {
            // 已選取功能鍵 (Bit 5 = 0)
            keys &= self.action_keys;
        }

        upper | keys
    }

    pub fn write_register(&mut self, value: u8) {
        let old_res = self.read_register();

        // 只允許寫入位元 4 和 5
        self.select = value & 0x30;

        let new_res = self.read_register();
        let should_trigger_interrupt = (old_res & !new_res & 0x0F) != 0;
        if should_trigger_interrupt && let Some(handler_ptr) = self.interrupt_handler {
            unsafe {
                use crate::gameboy::InterruptType;
                (*handler_ptr).trigger_interrupt(InterruptType::Joypad);
            }
        }
    }

    // 更新按鍵狀態 (由外部轉送，如 SDL3)
    // 按下時 bit 設為 0，放開時設為 1，返回是否觸發中斷
    pub fn set_key(&mut self, key: JoypadKey, pressed: bool) -> bool {
        let key_index = key.as_index();

        let key_state = &mut self.key_states[key_index];

        // 更新狀態追蹤
        let previous_pressed = key_state.pressed;

        if pressed != previous_pressed {
            key_state.pressed = pressed;
        }

        // 更新舊的位元狀態以保持兼容性
        match key {
            JoypadKey::A => Self::update_key_bit(&mut self.action_keys, 0x01, pressed),
            JoypadKey::B => Self::update_key_bit(&mut self.action_keys, 0x02, pressed),
            JoypadKey::Select => Self::update_key_bit(&mut self.action_keys, 0x04, pressed),
            JoypadKey::Start => Self::update_key_bit(&mut self.action_keys, 0x08, pressed),
            JoypadKey::Right => Self::update_key_bit(&mut self.direction_keys, 0x01, pressed),
            JoypadKey::Left => Self::update_key_bit(&mut self.direction_keys, 0x02, pressed),
            JoypadKey::Up => Self::update_key_bit(&mut self.direction_keys, 0x04, pressed),
            JoypadKey::Down => Self::update_key_bit(&mut self.direction_keys, 0x08, pressed),
        }

        // 真機上 Joypad interrupt 會在「新按下」時喚醒 CPU，
        // 不應該依賴當下 select row，否則 select=0x30 時無法被按鍵喚醒。
        let should_trigger_interrupt = pressed && !previous_pressed;
        if should_trigger_interrupt && let Some(handler_ptr) = self.interrupt_handler {
            unsafe {
                use crate::gameboy::InterruptType;
                (*handler_ptr).trigger_interrupt(InterruptType::Joypad);
            }
        }

        should_trigger_interrupt
    }
}

#[cfg(test)]
mod tests {
    use super::{Joypad, JoypadKey};
    use crate::gameboy::InterruptHandler;

    #[test]
    fn pressing_selected_key_requests_joypad_interrupt_immediately() {
        let mut joypad = Joypad::new();
        let mut interrupt_handler = InterruptHandler::new();
        joypad.set_interrupt_handler(&mut interrupt_handler as *mut InterruptHandler);

        joypad.write_register(0x20);
        joypad.set_key(JoypadKey::Right, true);

        assert_eq!(interrupt_handler.if_register & 0x10, 0x10);
    }

    #[test]
    fn selecting_pressed_key_row_requests_joypad_interrupt() {
        let mut joypad = Joypad::new();
        let mut interrupt_handler = InterruptHandler::new();
        joypad.set_interrupt_handler(&mut interrupt_handler as *mut InterruptHandler);

        joypad.set_key(JoypadKey::Start, true);
        assert_eq!(interrupt_handler.if_register & 0x10, 0x10);

        interrupt_handler.if_register = 0xE0;

        joypad.write_register(0x10);

        assert_eq!(interrupt_handler.if_register & 0x10, 0x10);
    }

    #[test]
    fn pressing_key_without_selected_row_still_requests_joypad_interrupt() {
        let mut joypad = Joypad::new();
        let mut interrupt_handler = InterruptHandler::new();
        joypad.set_interrupt_handler(&mut interrupt_handler as *mut InterruptHandler);

        assert_eq!(joypad.read_register(), 0xFF);

        joypad.set_key(JoypadKey::Down, true);

        assert_eq!(interrupt_handler.if_register & 0x10, 0x10);
        assert_eq!(joypad.read_register(), 0xFF);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum JoypadKey {
    A,
    B,
    Select,
    Start,
    Right,
    Left,
    Up,
    Down,
}

impl JoypadKey {
    /// 將 JoypadKey 轉換為數組索引
    pub fn as_index(self) -> usize {
        match self {
            JoypadKey::A => 0,
            JoypadKey::B => 1,
            JoypadKey::Select => 2,
            JoypadKey::Start => 3,
            JoypadKey::Right => 4,
            JoypadKey::Left => 5,
            JoypadKey::Up => 6,
            JoypadKey::Down => 7,
        }
    }
}
