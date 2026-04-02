// Timer (計時器) - 負責處理 Game Boy 的定時中斷

pub struct Timer {
    pub div: u16, // 內部分頻器 (高 8 位元即為 0xFF04 的 DIV 寄存器)
    pub tima: u8, // 0xFF05 - Timer Counter
    pub tma: u8,  // 0xFF06 - Timer Modulo
    pub tac: u8,  // 0xFF07 - Timer Control

    // 用於追蹤 TIMA 溢出後的延遲重載
    pub overflow_cycles: u8,
    pub pending_overflow: bool,
}

impl Timer {
    pub fn new() -> Self {
        Timer {
            div: 0xAB00, // 初始值通常不為 0
            tima: 0,
            tma: 0,
            tac: 0xF8, // 高位元讀取時通常為 1
            overflow_cycles: 0,
            pending_overflow: false,
        }
    }

    // 獲取當前 TAC 選擇的 DIV 位元
    fn get_timer_bit(&self) -> u16 {
        let bit_pos = match self.tac & 0x03 {
            0 => 9, // 4096 Hz (每 1024 T-cycles)
            1 => 3, // 262144 Hz (每 16 T-cycles)
            2 => 5, // 65536 Hz (每 64 T-cycles)
            3 => 7, // 16384 Hz (每 256 T-cycles)
            _ => unreachable!(),
        };
        (self.div >> bit_pos) & 0x01
    }

    // 檢測 falling edge 並更新 TIMA
    fn check_falling_edge(&mut self, old_bit: u16, new_bit: u16, interrupt_flags: &mut u8) -> bool {
        // Timer 必須啟用且發生 falling edge (1 -> 0)
        if old_bit == 1 && new_bit == 0 {
            self.increment_tima(interrupt_flags);
            true
        } else {
            false
        }
    }

    // 增加 TIMA
    fn increment_tima(&mut self, _interrupt_flags: &mut u8) {
        let (new_tima, overflow) = self.tima.overflowing_add(1);
        if overflow {
            // TIMA 溢出，設置為 0x00 並標記待處理
            // 實際上 TIMA 會在溢出後的下一個 M-cycle (4 T-cycles) 重載為 TMA
            self.tima = 0x00;
            self.pending_overflow = true;
            self.overflow_cycles = 4;
        } else {
            self.tima = new_tima;
        }
    }

    // 每個 T-狀態 (4.194MHz) 調用一次
    pub fn tick(&mut self, interrupt_flags: &mut u8) -> bool {
        let mut tima_incremented = false;

        // 處理溢出延遲
        if self.pending_overflow {
            self.overflow_cycles -= 1;
            if self.overflow_cycles == 0 {
                self.pending_overflow = false;
                // 觸發 Timer 中斷並重載 TIMA
                *interrupt_flags |= 0x04;
                self.tima = self.tma;
            }
        }

        // 計算舊的有效位元 (TAC enable AND DIV bit)
        let timer_enabled = (self.tac & 0x04) != 0;
        let old_bit = if timer_enabled { self.get_timer_bit() } else { 0 };

        // DIV 總是增加
        self.div = self.div.wrapping_add(1);

        // 計算新的有效位元
        let new_bit = if timer_enabled { self.get_timer_bit() } else { 0 };

        // 檢測 falling edge
        if timer_enabled {
            tima_incremented = self.check_falling_edge(old_bit, new_bit, interrupt_flags);
        }

        tima_incremented
    }

    pub fn read_register(&self, addr: u16) -> u8 {
        match addr {
            0xFF04 => (self.div >> 8) as u8,
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => self.tac | 0xF8,
            _ => 0xFF,
        }
    }

    pub fn write_register(&mut self, addr: u16, value: u8, interrupt_flags: &mut u8) {
        match addr {
            0xFF04 => {
                // 寫入 DIV 時需要檢測 falling edge
                let timer_enabled = (self.tac & 0x04) != 0;
                let old_bit = if timer_enabled { self.get_timer_bit() } else { 0 };
                
                // 寫入任何值都會將 DIV 清零
                self.div = 0;
                
                // 清零後位元變為 0
                if timer_enabled && old_bit == 1 {
                    self.increment_tima(interrupt_flags);
                }
            }
            0xFF05 => {
                // 寫入 TIMA 時，如果在溢出延遲期間，取消溢出
                if self.pending_overflow {
                    self.pending_overflow = false;
                }
                self.tima = value;
            }
            0xFF06 => {
                self.tma = value;
                // 如果在溢出延遲期間寫入 TMA，新值會被使用
            }
            0xFF07 => {
                // 寫入 TAC 時需要檢測 falling edge
                let old_enabled = (self.tac & 0x04) != 0;
                let old_bit = if old_enabled { self.get_timer_bit() } else { 0 };
                
                self.tac = value & 0x07;
                
                let new_enabled = (self.tac & 0x04) != 0;
                let new_bit = if new_enabled { self.get_timer_bit() } else { 0 };
                
                // 如果舊的有效位元是 1，新的是 0，觸發 TIMA 增加
                // 有效位元 = timer_enabled AND div_bit
                let old_effective = if old_enabled { old_bit } else { 0 };
                let new_effective = if new_enabled { new_bit } else { 0 };
                
                if old_effective == 1 && new_effective == 0 {
                    self.increment_tima(interrupt_flags);
                }
            }
            _ => {}
        }
    }
}
