// PPU (Picture Processing Unit) - Game Boy 圖形處理器

use crate::mmu::EnableState;

/// 精靈大小
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpriteSize {
    /// 8x8 像素
    Size8x8,
    /// 8x16 像素
    Size8x16,
}

#[derive(Debug, Clone, Copy)]
pub struct Sprite {
    pub y_pos: u8,      // Y 位置 (實際位置 = y_pos - 16)
    pub x_pos: u8,      // X 位置 (實際位置 = x_pos - 8)
    pub tile_index: u8, // 圖塊索引
    pub attributes: u8, // 屬性字節
}

impl Sprite {
    fn new(y_pos: u8, x_pos: u8, tile_index: u8, attributes: u8) -> Self {
        Sprite {
            y_pos,
            x_pos,
            tile_index,
            attributes,
        }
    }

    // 獲取精靈的實際 Y 位置
    fn actual_y(&self) -> i16 {
        self.y_pos as i16 - 16
    }
}

pub struct Ppu {
    // LCD 控制寄存器
    pub lcdc: u8, // 0xFF40 - LCD 控制
    pub stat: u8, // 0xFF41 - LCD 狀態
    pub scy: u8,  // 0xFF42 - 背景滾動 Y
    pub scx: u8,  // 0xFF43 - 背景滾動 X
    pub ly: u8,   // 0xFF44 - 當前掃描線
    pub lyc: u8,  // 0xFF45 - LY 比較
    pub dma: u8,  // 0xFF46 - DMA 傳輸
    pub bgp: u8,  // 0xFF47 - 背景調色板
    pub obp0: u8, // 0xFF48 - 精靈調色板 0
    pub obp1: u8, // 0xFF49 - 精靈調色板 1
    pub wy: u8,   // 0xFF4A - 視窗 Y 位置
    pub wx: u8,   // 0xFF4B - 視窗 X 位置

    // 內部狀態
    pub mode: LcdMode, // 當前 LCD 模式
    pub dots: u16,     // 點計數器

    // OAM 搜索結果 - 當前掃描線的可見精靈 (最多 10 個)
    pub oam_sprites: Vec<(usize, Sprite)>, // (OAM索引, 精靈)

    // 畫面緩衝區 - 160x144 像素，每個像素 2 位元 (0-3)
    pub framebuffer: Vec<u8>,

    // 用於呈現的穩定幀緩衝（在進入 VBlank 時快照）
    present_buffer: Vec<u8>,

    // 內部狀態追蹤 - 用於 STAT 中斷升緣觸發檢測
    pub prev_stat_irq: Option<()>,

    // 視窗內部行計數器 - 追蹤已渲染的視窗行數
    pub window_line_counter: u8,
    // 視窗是否在當前幀被觸發過
    pub window_triggered: bool,

    // 幀完成旗標：在進入 VBlank 時置位，供外部同步顯示
    frame_ready: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LcdMode {
    HBlank = 0,        // 水平空白期
    VBlank = 1,        // 垂直空白期
    OamSearch = 2,     // OAM 搜索
    PixelTransfer = 3, // 像素傳輸
}

impl Ppu {
    pub fn new() -> Self {
        Ppu {
            lcdc: 0x91, // 預設值
            stat: 0x85, // 預設值
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            dma: 0,
            bgp: 0xE4,  // 預設背景調色板 (0xE4 是標準 DMG 啟動值)
            obp0: 0xFF, // 預設精靈調色板 0
            obp1: 0xFF, // 預設精靈調色板 1
            wy: 0,
            wx: 0,
            mode: LcdMode::OamSearch,
            dots: 0,
            oam_sprites: Vec::new(),
            framebuffer: vec![0; 160 * 144], // 160x144 像素
            present_buffer: vec![0; 160 * 144],
            prev_stat_irq: None,
            window_line_counter: 0,
            window_triggered: false,
            frame_ready: false,
        }
    }

    // 讀取並清除幀完成旗標
    pub fn take_frame_ready(&mut self) -> bool {
        let ready = self.frame_ready;
        self.frame_ready = false;
        ready
    }

    pub fn get_present_framebuffer(&self) -> &[u8] {
        &self.present_buffer
    }

    // 讀取 LCD 寄存器
    pub fn read_register(&self, addr: u16) -> u8 {
        match addr {
            0xFF40 => self.lcdc,
            0xFF41 => self.stat | (self.mode as u8),
            0xFF42 => self.scy,
            0xFF43 => self.scx,
            0xFF44 => self.ly,
            0xFF45 => self.lyc,
            0xFF46 => self.dma,
            0xFF47 => self.bgp,
            0xFF48 => self.obp0,
            0xFF49 => self.obp1,
            0xFF4A => self.wy,
            0xFF4B => self.wx,
            _ => 0xFF,
        }
    }

    // 寫入 LCD 寄存器
    pub fn write_register(&mut self, addr: u16, value: u8, interrupt_flags: &mut u8) {
        match addr {
            0xFF40 => {
                let old_lcdc = self.lcdc;
                self.lcdc = value;
                match ((old_lcdc & 0x80) != 0, (value & 0x80) != 0) {
                    (true, false) => {
                        self.ly = 0;
                        self.dots = 0;
                        self.mode = LcdMode::HBlank;
                        self.stat = (self.stat & 0xFC) | (LcdMode::HBlank as u8);
                        self.framebuffer.fill(0);
                    }
                    (false, true) => {
                        self.ly = 0;
                        self.dots = 0;
                        self.mode = LcdMode::OamSearch;
                        self.stat = (self.stat & 0xFC) | (LcdMode::OamSearch as u8);
                    }
                    _ => {}
                }
            }
            0xFF41 => {
                // 位 0-2 只讀，位 7 始終為 1
                self.stat = (self.stat & 0x07) | (value & 0x78) | 0x80;
                self.update_stat(interrupt_flags);
            }
            0xFF42 => self.scy = value,
            0xFF43 => self.scx = value,
            0xFF44 => {} // LY 是只讀的
            0xFF45 => {
                self.lyc = value;
                self.update_stat(interrupt_flags);
            }
            0xFF46 => self.dma = value,
            0xFF47 => self.bgp = value,
            0xFF48 => self.obp0 = value,
            0xFF49 => self.obp1 = value,
            0xFF4A => self.wy = value,
            0xFF4B => self.wx = value,
            _ => {}
        }
    }

    fn update_stat(&mut self, interrupt_flags: &mut u8) {
        // 更新 LYC == LY 標誌 (Bit 2)
        if self.ly == self.lyc {
            self.stat |= 0x04;
        } else {
            self.stat &= !0x04;
        }

        // 檢查 STAT 中斷條件 - 使用 Option 代替 bool
        let mut irq: Option<()> = None;

        // LYC == LY 中斷 (Bit 6)
        if (self.stat & 0x40) != 0 && (self.stat & 0x04) != 0 {
            irq = Some(());
        }

        // Mode 中斷
        match self.mode {
            LcdMode::HBlank => {
                if (self.stat & 0x08) != 0 {
                    irq = Some(());
                }
            }
            LcdMode::VBlank => {
                if (self.stat & 0x10) != 0 {
                    irq = Some(());
                }
            }
            LcdMode::OamSearch => {
                if (self.stat & 0x20) != 0 {
                    irq = Some(());
                }
            }
            _ => {}
        }

        // 升緣觸發中斷 - 只在從無中斷變為有中斷時觸發
        if irq.is_some() && self.prev_stat_irq.is_none() {
            *interrupt_flags |= 0x02; // LCD STAT 中斷 (Bit 1)
        }
        self.prev_stat_irq = irq;
    }

    fn change_mode(&mut self, mode: LcdMode, interrupt_flags: &mut u8) {
        self.mode = mode;
        self.stat = (self.stat & 0xFC) | (mode as u8);

        if mode == LcdMode::OamSearch {
            self.oam_sprites.clear();
        }

        self.update_stat(interrupt_flags);
    }

    // 從 OAM 讀取精靈資料
    fn read_sprite_data(&self, mmu: &crate::mmu::Mmu, sprite_index: usize) -> (u8, u8, u8, u8) {
        let base_addr = 0xFE00 + (sprite_index * 4) as u16;
        let y_pos = mmu.read_byte_ppu(base_addr);
        let x_pos = mmu.read_byte_ppu(base_addr + 1);
        let tile_index = mmu.read_byte_ppu(base_addr + 2);
        let attributes = mmu.read_byte_ppu(base_addr + 3);

        (y_pos, x_pos, tile_index, attributes)
    }

    // 檢查精靈在當前掃描線上的可見性
    fn check_sprite_visibility(&mut self, mmu: &crate::mmu::Mmu, sprite_index: usize) {
        if self.oam_sprites.len() >= 10 {
            return; // 每條掃描線最多 10 個精靈
        }

        let (y_pos, x_pos, tile_index, attributes) = self.read_sprite_data(mmu, sprite_index);
        let sprite = Sprite::new(y_pos, x_pos, tile_index, attributes);

        let sprite_height = match self.get_sprite_size() {
            SpriteSize::Size8x8 => 8,
            SpriteSize::Size8x16 => 16,
        };
        let sprite_y = sprite.actual_y();

        // OAM 搜索只根據 Y 座標判斷，x_pos 不影響搜索
        // x_pos = 0 的精靈仍計入 10 個精靈限制
        if self.ly as i16 >= sprite_y && (self.ly as i16) < sprite_y + sprite_height as i16 {
            self.oam_sprites.push((sprite_index, sprite));
        }
    }

    // 在 OAM 搜索結束後對精靈進行排序
    fn sort_sprites(&mut self) {
        // DMG 優先級規則：
        // 1. X 座標較小的精靈優先（在前面）
        // 2. X 座標相同時，OAM 索引較小的優先
        self.oam_sprites.sort_by(|a, b| {
            let x_cmp = a.1.x_pos.cmp(&b.1.x_pos);
            if x_cmp == std::cmp::Ordering::Equal {
                a.0.cmp(&b.0) // OAM 索引
            } else {
                x_cmp
            }
        });
    }

    // 檢查 LCD 是否啟用
    fn lcd_state(&self) -> EnableState {
        if (self.lcdc & 0x80) != 0 {
            EnableState::Enabled
        } else {
            EnableState::Disabled
        }
    }

    // 獲取精靈大小
    pub fn get_sprite_size(&self) -> SpriteSize {
        match self.lcdc & 0x04 {
            0 => SpriteSize::Size8x8,
            _ => SpriteSize::Size8x16,
        }
    }

    // PPU 主時鐘滴答 - 每個 T-狀態調用一次
    pub fn tick(&mut self, mmu: &crate::mmu::Mmu, interrupt_flags: &mut u8) {
        if self.lcd_state() == EnableState::Disabled {
            return;
        }

        self.dots = self.dots.wrapping_add(1);

        match self.mode {
            LcdMode::OamSearch => {
                // 每 2 個點檢查一個精靈
                // dots 從 1 開始，所以 dots=1,2 檢查 sprite 0，dots=3,4 檢查 sprite 1，以此類推
                if self.dots.is_multiple_of(2) {
                    let sprite_index = (self.dots / 2 - 1) as usize;
                    if sprite_index < 40 {
                        self.check_sprite_visibility(mmu, sprite_index);
                    }
                }

                if self.dots >= 80 {
                    // OAM 搜索結束後排序精靈
                    self.sort_sprites();
                    self.change_mode(LcdMode::PixelTransfer, interrupt_flags);
                }
            }
            LcdMode::PixelTransfer => {
                if self.dots >= 252 {
                    self.render_scanline(mmu);
                    self.change_mode(LcdMode::HBlank, interrupt_flags);
                }
            }
            LcdMode::HBlank => {
                if self.dots >= 456 {
                    self.dots = 0;
                    self.ly += 1;
                    self.update_stat(interrupt_flags);

                    if self.ly >= 144 {
                        self.change_mode(LcdMode::VBlank, interrupt_flags);
                        *interrupt_flags |= 0x01; // VBlank 中斷
                        self.present_buffer.copy_from_slice(&self.framebuffer);
                        self.frame_ready = true;
                    } else {
                        self.change_mode(LcdMode::OamSearch, interrupt_flags);
                    }
                }
            }
            LcdMode::VBlank => {
                if self.dots >= 456 {
                    self.dots = 0;
                    self.ly += 1;
                    self.update_stat(interrupt_flags);

                    if self.ly >= 154 {
                        self.ly = 0;
                        self.window_line_counter = 0;
                        self.window_triggered = false;
                        self.change_mode(LcdMode::OamSearch, interrupt_flags);
                    }
                }
            }
        }
    }

    // 渲染當前掃描線 - 背景 + 視窗 + 精靈
    fn render_scanline(&mut self, mmu: &crate::mmu::Mmu) {
        if self.ly >= 144 {
            return;
        }

        let ly = self.ly;
        let scx = self.scx;
        let scy = self.scy;
        let wy = self.wy;
        let wx = self.wx;
        let lcdc = self.lcdc;
        let bgp = self.bgp;
        let obp0 = self.obp0;
        let obp1 = self.obp1;

        let bg_enabled = (lcdc & 0x01) != 0;
        let sprite_enabled = (lcdc & 0x02) != 0;
        let window_enabled = (lcdc & 0x20) != 0;
        let sprite_size_16 = (lcdc & 0x04) != 0;

        // 更新視窗觸發鎖存器 (WY condition)
        // 一旦 LY >= WY 且視窗/BG 啟用，鎖存器設定並保持到幀結束
        if window_enabled && bg_enabled && ly >= wy {
            self.window_triggered = true;
        }

        // 檢查此掃描線是否渲染視窗：鎖存器已設定 且 視窗當前啟用
        let render_window = self.window_triggered && window_enabled && bg_enabled;

        let bg_map_base: u16 = if (lcdc & 0x08) != 0 { 0x9C00 } else { 0x9800 };
        let win_map_base: u16 = if (lcdc & 0x40) != 0 { 0x9C00 } else { 0x9800 };
        let tile_data_8000 = (lcdc & 0x10) != 0;

        let window_line = self.window_line_counter;
        let base_offset = ly as usize * 160;
        let mut window_used_this_line = false;

        // 克隆精靈列表以避免借用衝突
        let sprites = self.oam_sprites.clone();

        for x in 0..160u8 {
            let mut bg_color_idx: u8 = 0;
            let mut final_color: u8 = 0;
            let mut used_window = false;

            if bg_enabled {
                // 檢查此像素是否在視窗內
                if render_window && (x as i16 >= wx as i16 - 7) {
                    // 視窗像素
                    let win_x = (x as i16 - (wx as i16 - 7)) as u8;
                    let win_y = window_line;

                    let tile_x = win_x / 8;
                    let tile_y = win_y / 8;
                    let tile_addr = win_map_base + (tile_y as u16 * 32) + tile_x as u16;
                    let tile_index = mmu.read_byte_ppu(tile_addr);

                    let tile_data_addr = if tile_data_8000 {
                        0x8000u16 + (tile_index as u16 * 16)
                    } else {
                        let signed_index = tile_index as i8 as i32;
                        (0x9000i32 + (signed_index * 16)) as u16
                    };

                    let tile_line = (win_y % 8) as u16;
                    let line_addr = tile_data_addr + (tile_line * 2);
                    let low_byte = mmu.read_byte_ppu(line_addr);
                    let high_byte = mmu.read_byte_ppu(line_addr + 1);

                    let pixel_x = win_x % 8;
                    let bit_index = 7 - pixel_x;
                    let low_bit = (low_byte >> bit_index) & 0x01;
                    let high_bit = (high_byte >> bit_index) & 0x01;
                    bg_color_idx = (high_bit << 1) | low_bit;

                    let shift = bg_color_idx * 2;
                    final_color = (bgp >> shift) & 0x03;
                    used_window = true;
                } else {
                    // 背景像素
                    let bg_x = x.wrapping_add(scx);
                    let bg_y = ly.wrapping_add(scy);

                    let tile_x = (bg_x / 8) as u16;
                    let tile_y = (bg_y / 8) as u16;
                    let tile_addr = bg_map_base + (tile_y * 32) + tile_x;
                    let tile_index = mmu.read_byte_ppu(tile_addr);

                    let tile_data_addr = if tile_data_8000 {
                        0x8000u16 + (tile_index as u16 * 16)
                    } else {
                        let signed_index = tile_index as i8 as i32;
                        (0x9000i32 + (signed_index * 16)) as u16
                    };

                    let tile_line = (bg_y % 8) as u16;
                    let line_addr = tile_data_addr + (tile_line * 2);
                    let low_byte = mmu.read_byte_ppu(line_addr);
                    let high_byte = mmu.read_byte_ppu(line_addr + 1);

                    let pixel_x = bg_x % 8;
                    let bit_index = 7 - pixel_x;
                    let low_bit = (low_byte >> bit_index) & 0x01;
                    let high_bit = (high_byte >> bit_index) & 0x01;
                    bg_color_idx = (high_bit << 1) | low_bit;

                    let shift = bg_color_idx * 2;
                    final_color = (bgp >> shift) & 0x03;
                }
            }

            if used_window {
                window_used_this_line = true;
            }

            // 精靈渲染
            if sprite_enabled {
                let sprite_height: i16 = if sprite_size_16 { 16 } else { 8 };

                for (_, sprite) in sprites.iter() {
                    let sprite_x = sprite.x_pos as i16 - 8;

                    // 檢查像素是否在精靈 X 範圍內
                    if (x as i16) < sprite_x || (x as i16) >= sprite_x + 8 {
                        continue;
                    }

                    // x_pos == 0 的精靈完全在螢幕外，不渲染
                    if sprite.x_pos == 0 {
                        continue;
                    }

                    // 計算精靈內部相對座標
                    let rel_x = (x as i16 - sprite_x) as u8;
                    let mut rel_y = (ly as i16 - (sprite.y_pos as i16 - 16)) as u8;

                    // 垂直翻轉
                    if (sprite.attributes & 0x40) != 0 {
                        rel_y = (sprite_height as u8) - 1 - rel_y;
                    }

                    // 確定圖塊索引 (8x16 模式下 bit 0 被忽略)
                    let tile_index = if sprite_size_16 {
                        if rel_y >= 8 {
                            sprite.tile_index | 0x01
                        } else {
                            sprite.tile_index & 0xFE
                        }
                    } else {
                        sprite.tile_index
                    };

                    // 獲取圖塊資料（精靈總是使用 0x8000 定址）
                    let tile_line = (rel_y % 8) as u16;
                    let tile_addr = 0x8000u16 + (tile_index as u16 * 16) + (tile_line * 2);
                    let low_byte = mmu.read_byte_ppu(tile_addr);
                    let high_byte = mmu.read_byte_ppu(tile_addr + 1);

                    // 提取像素顏色 - 處理水平翻轉
                    let bit_index = if (sprite.attributes & 0x20) != 0 {
                        rel_x
                    } else {
                        7 - rel_x
                    };
                    let low_bit = (low_byte >> bit_index) & 0x01;
                    let high_bit = (high_byte >> bit_index) & 0x01;
                    let sprite_color_idx = (high_bit << 1) | low_bit;

                    // 跳過透明像素
                    if sprite_color_idx == 0 {
                        continue;
                    }

                    // 檢查優先級：BehindBg 且背景非透明時，背景優先
                    if (sprite.attributes & 0x80) != 0 && bg_color_idx != 0 {
                        break;
                    }

                    // 應用精靈調色板
                    let palette = if (sprite.attributes & 0x10) != 0 {
                        obp1
                    } else {
                        obp0
                    };
                    let shift = sprite_color_idx * 2;
                    final_color = (palette >> shift) & 0x03;
                    break;
                }
            }

            self.framebuffer[base_offset + x as usize] = final_color;
        }

        if window_used_this_line {
            self.window_line_counter += 1;
        }
    }

    // 獲取畫面緩衝區的引用
    #[allow(dead_code)]
    pub fn get_framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }
}
