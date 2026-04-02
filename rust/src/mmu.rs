// 記憶體管理單元 (MMU) - 負責 CPU 與記憶體/I/O 的通訊
// 整合操作碼資料和記憶體映射

use crate::cpu; // 引用 cpu 模組
use crate::ppu::{LcdMode, Ppu};
use crate::rom; // 引用 rom 模組

#[allow(dead_code)]
pub trait Memory {
    fn read_byte(&self, address: u16) -> u8;
    fn write_byte(&mut self, address: u16, value: u8);
    fn read_word(&self, address: u16) -> u16;
    fn write_word(&mut self, address: u16, value: u16);
}

pub trait IoHandler {
    fn read_io(&self, address: u16) -> u8;
    fn write_io(&mut self, address: u16, value: u8, interrupt_flags: &mut u8);
}

/// 功能啟用狀態
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnableState {
    Enabled,
    Disabled,
}

pub struct Mmu {
    pub rom: Vec<u8>,          // ROM 數據 (包含所有銀行)
    pub wram: [u8; 8192],      // WRAM - 8KB 內部工作 RAM
    pub ext_ram: Vec<u8>,      // 外部卡帶 RAM (根據 MBC 分頁)
    pub vram: Vec<u8>,         // VRAM - 8KB 視訊 RAM
    pub oam: Vec<u8>,          // OAM - 160 位元組物件屬性記憶體
    pub hram: [u8; 127],       // HRAM - 127 位元組高位 RAM
    pub ie: u8,                // 中斷啟用寄存器
    pub if_reg: u8,            // 中斷標誌寄存器 (0xFF0F)
    pub serial_data: u8,       // 專用的串口數據寄存器 (SB)
    pub serial_control: u8,    // 專用的串口控制寄存器 (SC)
    pub serial_output: String, // 串口輸出緩衝區 (用於測試 ROM)

    // MBC (Memory Bank Controller) 相關狀態
    pub mbc_type: u8,
    pub rom_bank: u16,
    pub ram_bank: u8,
    pub ram_state: EnableState,
    pub banking_mode: u8, // 0 = ROM banking, 1 = RAM banking

    io_handler: Option<Box<dyn IoHandler>>,

    // 供 CPU-side VRAM/OAM 存取限制使用（PPU 內部讀取不受限）
    ppu: Option<*const Ppu>,
}

impl Mmu {
    pub fn new() -> Self {
        Mmu {
            rom: vec![0; 0x8000],
            wram: [0; 0x2000],
            ext_ram: Vec::new(),
            vram: vec![0; 0x2000], // 8KB VRAM
            oam: vec![0; 0xA0],    // 160 位元組 OAM
            hram: [0; 127],
            ie: 0,
            if_reg: 0xE0,
            serial_data: 0,
            serial_control: 0x7E, // SC 預設值
            serial_output: String::new(),

            mbc_type: 0,
            rom_bank: 1,
            ram_bank: 0,
            ram_state: EnableState::Disabled,
            banking_mode: 0,

            io_handler: None,
            ppu: None,
        }
    }

    // 設置 I/O 處理器
    pub fn set_io_handler(&mut self, handler: Box<dyn IoHandler>) {
        self.io_handler = Some(handler);
    }

    pub fn set_ppu(&mut self, ppu: &Ppu) {
        self.ppu = Some(std::ptr::from_ref(ppu));
    }

    // 獲取操作碼引用 (現在改為引用全域靜態變數)
    #[allow(dead_code)]
    pub fn get_opcodes(&self) -> &cpu::Opcodes {
        &cpu::OPCODES
    }
}

impl Memory for Mmu {
    fn read_byte(&self, address: u16) -> u8 {
        self.read_byte(address)
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        self.write_byte(address, value);
    }

    fn read_word(&self, address: u16) -> u16 {
        let low = self.read_byte(address);
        let high = self.read_byte(address + 1);
        ((high as u16) << 8) | (low as u16)
    }

    fn write_word(&mut self, address: u16, value: u16) {
        let low = (value & 0xFF) as u8;
        let high = ((value >> 8) & 0xFF) as u8;
        self.write_byte(address, low);
        self.write_byte(address + 1, high);
    }
}

impl Mmu {
    // 給 PPU/DMA 內部使用：不受 CPU-side VRAM/OAM 存取限制影響
    pub fn read_byte_ppu(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x3FFF => self.rom[address as usize], // ROM Bank 0
            0x4000..=0x7FFF => {
                // ROM Bank 1-N (MBC1)
                let bank = if self.mbc_type == 0 { 1 } else { self.rom_bank };
                let addr = (bank as usize * 0x4000) + (address as usize - 0x4000);
                self.rom[addr % self.rom.len()]
            }
            0x8000..=0x9FFF => self.vram[(address - 0x8000) as usize], // VRAM
            0xA000..=0xBFFF => {
                // 外部 RAM
                if self.ram_state == EnableState::Enabled && !self.ext_ram.is_empty() {
                    let addr = (self.ram_bank as usize * 0x2000) + (address as usize - 0xA000);
                    self.ext_ram[addr % self.ext_ram.len()]
                } else {
                    0xFF // 或者 0xFF，取決於硬體行為，通常未連接時回傳 0xFF
                }
            }
            0xC000..=0xDFFF => self.wram[(address - 0xC000) as usize], // WRAM
            0xE000..=0xFDFF => self.wram[(address - 0xE000) as usize], // Echo RAM
            0xFE00..=0xFE9F => self.oam[(address - 0xFE00) as usize],  // OAM
            0xFEA0..=0xFEFF => 0xFF,                                   // 未使用區域
            0xFF00..=0xFF7F => {
                match address {
                    0xFF01 => self.serial_data,
                    0xFF02 => self.serial_control | 0x7E,
                    0xFF0F => self.if_reg | 0xE0, // 高 3 位始終為 1
                    _ => {
                        if let Some(ref handler) = self.io_handler {
                            handler.as_ref().read_io(address)
                        } else {
                            0
                        }
                    }
                }
            } // I/O 寄存器
            0xFF80..=0xFFFE => self.hram[(address - 0xFF80) as usize], // HRAM
            0xFFFF => self.ie,                                         // IE
        }
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        // CPU-side VRAM/OAM 存取限制：
        // - Mode 3 (PixelTransfer) 時，CPU 不能存取 VRAM
        // - Mode 2/3 時，CPU 不能存取 OAM
        if let Some(ppu_ptr) = self.ppu {
            unsafe {
                let lcd_enabled = ((*ppu_ptr).lcdc & 0x80) != 0;
                if lcd_enabled {
                    match address {
                        0x8000..=0x9FFF => {
                            if (*ppu_ptr).mode == LcdMode::PixelTransfer {
                                return 0xFF;
                            }
                        }
                        0xFE00..=0xFE9F => {
                            if matches!(
                                (*ppu_ptr).mode,
                                LcdMode::OamSearch | LcdMode::PixelTransfer
                            ) {
                                return 0xFF;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        self.read_byte_ppu(address)
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        // CPU-side VRAM/OAM 存取限制（同 read_byte 的規則）
        if let Some(ppu_ptr) = self.ppu {
            unsafe {
                let lcd_enabled = ((*ppu_ptr).lcdc & 0x80) != 0;
                if lcd_enabled {
                    match address {
                        0x8000..=0x9FFF => {
                            if (*ppu_ptr).mode == LcdMode::PixelTransfer {
                                return;
                            }
                        }
                        0xFE00..=0xFE9F => {
                            if matches!(
                                (*ppu_ptr).mode,
                                LcdMode::OamSearch | LcdMode::PixelTransfer
                            ) {
                                return;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        match address {
            0x0000..=0x1FFF => {
                // MBC1: RAM Enable
                if self.mbc_type == 1 {
                    self.ram_state = if (value & 0x0F) == 0x0A {
                        EnableState::Enabled
                    } else {
                        EnableState::Disabled
                    };
                }
            }
            0x2000..=0x3FFF => {
                // MBC1: ROM Bank Number
                if self.mbc_type == 1 {
                    let mut bank = (value & 0x1F) as u16;
                    if bank == 0 {
                        bank = 1;
                    }
                    self.rom_bank = (self.rom_bank & 0x60) | bank;
                }
            }
            0x4000..=0x5FFF => {
                // MBC1: RAM Bank Number / Upper ROM Bank Bits
                if self.mbc_type == 1 {
                    if self.banking_mode == 0 {
                        self.rom_bank = (self.rom_bank & 0x1F) | ((value as u16 & 0x03) << 5);
                    } else {
                        self.ram_bank = value & 0x03;
                    }
                }
            }
            0x6000..=0x7FFF => {
                // MBC1: Banking Mode Select
                if self.mbc_type == 1 {
                    self.banking_mode = value & 0x01;
                }
            }
            0x8000..=0x9FFF => {
                self.vram[(address - 0x8000) as usize] = value;
            } // VRAM
            0xA000..=0xBFFF => {
                // 外部 RAM
                if self.ram_state == EnableState::Enabled && !self.ext_ram.is_empty() {
                    let addr = (self.ram_bank as usize * 0x2000) + (address as usize - 0xA000);
                    let len = self.ext_ram.len();
                    self.ext_ram[addr % len] = value;
                }
            }
            0xC000..=0xDFFF => self.wram[(address - 0xC000) as usize] = value, // WRAM
            0xE000..=0xFDFF => self.wram[(address - 0xE000) as usize] = value, // Echo RAM
            0xFE00..=0xFE9F => self.oam[(address - 0xFE00) as usize] = value,  // OAM
            0xFEA0..=0xFEFF => {}                                              // 未使用
            0xFF00..=0xFF7F => {
                if address == 0xFF0F {
                    self.if_reg = value | 0xE0;
                } else if address == 0xFF01 {
                    // Serial Data (SB)
                    self.serial_data = value;
                } else if address == 0xFF02 {
                    // Serial Control (SC)
                    self.serial_control = value;
                    // 如果啟動了傳輸 (Bit 7 為 1)
                    if (value & 0x80) != 0 {
                        // 捕獲串口輸出 (用於測試 ROM)
                        let char_byte = self.serial_data;
                        if (0x20..0x7F).contains(&char_byte) {
                            self.serial_output.push(char_byte as char);
                        } else if char_byte == 0x0A {
                            self.serial_output.push('\n');
                        }
                        // 模擬傳輸完成：清除 Bit 7 並觸發 Serial 中斷 (Bit 3)
                        self.serial_control &= 0x7F;
                        self.if_reg |= 0x08;
                        // 模擬沒連接設備時，讀取回來的數據會是 0xFF
                        self.serial_data = 0xFF;
                    }
                } else if address == 0xFF46 {
                    // 執行 OAM DMA 傳輸
                    self.perform_dma(value);
                    // 同時更新 PPU 的暫存器
                    if let Some(ref mut handler) = self.io_handler {
                        let mut if_reg = self.if_reg;
                        handler.as_mut().write_io(address, value, &mut if_reg);
                        self.if_reg = if_reg;
                    }
                } else if let Some(ref mut handler) = self.io_handler {
                    let mut if_reg = self.if_reg;
                    handler.as_mut().write_io(address, value, &mut if_reg);
                    self.if_reg = if_reg;
                }
            } // I/O 寄存器
            0xFF80..=0xFFFE => self.hram[(address - 0xFF80) as usize] = value, // HRAM
            0xFFFF => self.ie = value,                                         // IE
        }
    }

    // 讀取字組 (little-endian)
    pub fn read_word(&self, address: u16) -> u16 {
        let low = self.read_byte(address);
        let high = self.read_byte(address + 1);
        ((high as u16) << 8) | (low as u16)
    }

    // 寫入字組 (little-endian)
    pub fn write_word(&mut self, address: u16, value: u16) {
        let low = (value & 0xFF) as u8;
        let high = ((value >> 8) & 0xFF) as u8;
        self.write_byte(address, low);
        self.write_byte(address + 1, high);
    }

    // 載入 ROM 資料
    pub fn load_rom(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let rom_data = rom::read_rom_file(path)?;
        self.load_rom_from_bytes(rom_data)
    }

    pub fn load_rom_from_bytes(
        &mut self,
        rom_data: Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 偵錯資訊：確認載入成功
        println!("成功載入 ROM 陣列 (大小: {} bytes)", rom_data.len());

        // 重新分配 self.rom 以處理不同大小的 ROM (MBC)
        self.rom = rom_data;

        // 檢查 MBC 與 RAM 大小
        if self.rom.len() > 0x149 {
            match self.rom[0x147] {
                1..=3 => self.mbc_type = 1,       // MBC1
                5..=6 => self.mbc_type = 2,       // MBC2
                0x0F..=0x13 => self.mbc_type = 3, // MBC3
                _ => self.mbc_type = 0,
            }

            let ram_size = match self.rom[0x149] {
                0x01 => 2 * 1024,
                0x02 => 8 * 1024,
                0x03 => 32 * 1024,
                0x04 => 128 * 1024,
                0x05 => 64 * 1024,
                _ => 0,
            };

            if ram_size > 0 {
                self.ext_ram = vec![0; ram_size];
                self.load_save_file();
            }
        }

        Ok(())
    }

    pub fn load_save_file(&mut self) {
        if let Ok(data) = std::fs::read("save.sav") {
            let len = data.len().min(self.ext_ram.len());
            self.ext_ram[..len].copy_from_slice(&data[..len]);
            println!("已載入存檔: save.sav ({} bytes)", len);
        }
    }

    pub fn save_external_ram(&self) {
        if !self.ext_ram.is_empty() {
            if let Err(e) = std::fs::write("save.sav", &self.ext_ram) {
                eprintln!("存檔失敗: {}", e);
            } else {
                println!("存檔成功: save.sav");
            }
        }
    }

    // 執行 OAM DMA 傳輸 (0xFF46)
    fn perform_dma(&mut self, value: u8) {
        let source_base = (value as u16) << 8;
        for i in 0..0xA0 {
            let byte = self.read_byte_ppu(source_base + i);
            self.oam[i as usize] = byte;
        }
    }
}
