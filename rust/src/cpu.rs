use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

/// 全域操作碼快取 (Rust 1.80.0+)
pub static OPCODES: LazyLock<Opcodes> =
    LazyLock::new(|| load_opcodes().expect("無法載入 Opcodes.json 檔案。請確保檔案存在且格式正確。"));

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Operand {
    pub name: String,
    #[serde(default)]
    pub bytes: Option<u8>,
    #[serde(default)]
    pub immediate: Option<bool>,
    #[serde(default)]
    pub increment: Option<bool>,
    #[serde(default)]
    pub decrement: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Flags {
    #[serde(rename = "Z")]
    pub z: String,
    #[serde(rename = "N")]
    pub n: String,
    #[serde(rename = "H")]
    pub h: String,
    #[serde(rename = "C")]
    pub c: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Opcode {
    pub mnemonic: String,
    pub bytes: u8,
    pub cycles: Vec<u8>,
    pub operands: Vec<Operand>,
    #[serde(default)]
    pub immediate: bool,
    pub flags: Flags,
}

pub struct Opcodes {
    pub unprefixed: Vec<Option<Opcode>>,
    pub cbprefixed: Vec<Option<Opcode>>,
}

/// CPU 運行狀態
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CpuState {
    Running, // 正常運行
    Halted,  // 暫停 (HALT)
}

/// 中斷主啟用狀態 (IME)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InterruptMasterState {
    Disabled, // 已禁用
    Pending,  // 準備啟用 (EI 指令後的延遲週期)
    Enabled,  // 已啟用
}

/// 旗標狀態
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlagState {
    Set,
    Clear,
}

// CPU 結構 - 包含 MMU
pub struct Cpu {
    pub pc: u16,                   // 程式計數器
    pub sp: u16,                   // 堆疊指標
    pub registers: Registers,      // 暫存器
    pub flags: CpuFlags,           // CPU 旗標
    pub state: CpuState,           // CPU 運行狀態
    pub ime: InterruptMasterState, // 中斷主啟用狀態
    pub instr_count: u64,          // 指令計數器 (用於除錯)
    pub branch_taken: bool,        // 條件分支是否成立 (用於正確計算週期)
    pub halt_bug: bool,            // HALT bug 標誌：下一次 fetch 不增加 PC
}

#[derive(Debug)]
pub struct Registers {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
}

#[derive(Debug)]
pub struct CpuFlags {
    pub z: FlagState, // Zero
    pub n: FlagState, // Negative
    pub h: FlagState, // Half Carry
    pub c: FlagState, // Carry
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            pc: 0x0100, // Game Boy 程式起始位址
            sp: 0xFFFE, // 堆疊起始位址
            registers: Registers {
                a: 0x01,
                f: 0xB0,
                b: 0x00,
                c: 0x13,
                d: 0x00,
                e: 0xD8,
                h: 0x01,
                l: 0x4D,
            },
            flags: CpuFlags {
                z: FlagState::Set,
                n: FlagState::Clear,
                h: FlagState::Set,
                c: FlagState::Set,
            },
            state: CpuState::Running,
            ime: InterruptMasterState::Disabled,
            instr_count: 0,
            branch_taken: false,
            halt_bug: false,
        }
    }

    // 讀取下一個位元組並前進 PC
    pub fn fetch_byte(&mut self, mmu: &mut crate::mmu::Mmu) -> u8 {
        let byte = mmu.read_byte(self.pc);
        // HALT bug: 如果設置了 halt_bug 標誌，不遞增 PC
        if self.halt_bug {
            self.halt_bug = false; // 只影響一次
        } else {
            self.pc = self.pc.wrapping_add(1);
        }
        byte
    }

    // 讀取下一個字並前進 PC
    pub fn fetch_word(&mut self, mmu: &mut crate::mmu::Mmu) -> u16 {
        let word = mmu.read_word(self.pc);
        self.pc = self.pc.wrapping_add(2);
        word
    }

    // 堆疊操作
    pub fn push_word(&mut self, mmu: &mut crate::mmu::Mmu, value: u16) {
        self.sp = self.sp.wrapping_sub(2);
        mmu.write_word(self.sp, value);
    }

    pub fn pop_word(&mut self, mmu: &crate::mmu::Mmu) -> u16 {
        let value = mmu.read_word(self.sp);
        self.sp = self.sp.wrapping_add(2);
        value
    }

    // 執行一個指令
    pub fn step(&mut self, mmu: &mut crate::mmu::Mmu) -> u32 {
        // --- 處理中斷 ---
        let ie = mmu.read_byte(0xFFFF);
        let mut iff = mmu.read_byte(0xFF0F);
        let interrupts = ie & iff & 0x1F;

        if interrupts != 0 {
            // 只要有中斷就會喚醒 CPU
            self.state = CpuState::Running;

            if self.ime == InterruptMasterState::Enabled {
                self.ime = InterruptMasterState::Disabled;

                // 找到最高優先級的中斷
                let interrupt_bit = interrupts.trailing_zeros() as u8;
                let vector = match interrupt_bit {
                    0 => 0x40, // VBlank
                    1 => 0x48, // LCD STAT
                    2 => 0x50, // Timer
                    3 => 0x58, // Serial
                    4 => 0x60, // Joypad
                    _ => unreachable!(),
                };

                // 清除對應的中斷標誌
                iff &= !(1 << interrupt_bit);
                mmu.write_byte(0xFF0F, iff);

                if interrupt_bit == 4 {
                    crate::gameboy::log_joypad_interrupt_service(self.pc, ie, iff);
                }

                // 推入當前 PC 到堆疊並跳轉
                let current_pc = self.pc;
                self.push_word(mmu, current_pc);
                self.pc = vector as u16;

                return 20; // 中斷處理固定消耗 20 週期
            }
        }

        if self.state == CpuState::Halted {
            return 4; // Halted 時每個指令週期消耗 4 週期
        }

        // 處理 EI 延遲生效：在 EI 指令之後的一個指令週期後啟用 IME
        if self.ime == InterruptMasterState::Pending {
            self.ime = InterruptMasterState::Enabled;
        }

        // --- 結束中斷處理 ---

        // 重置分支標誌
        self.branch_taken = false;

        let first_byte = self.fetch_byte(mmu);
        let pc_before = self.pc.wrapping_sub(1);

        // 查找操作碼 (直接引用全域靜態變數，避免借用 mmu)
        let opcode_opt = if first_byte == 0xCB {
            let second_byte = self.fetch_byte(mmu);
            OPCODES.cbprefixed[second_byte as usize].as_ref()
        } else {
            OPCODES.unprefixed[first_byte as usize].as_ref()
        };

        // 跟蹤指令計數器 - 全局可訪問
        self.instr_count += 1;

        if let Some(opcode) = opcode_opt {
            // 追蹤：偵測未實作指令
            let mnemonic = opcode.mnemonic.as_str();
            if !matches!(
                mnemonic,
                "LD" | "ADD"
                    | "ADC"
                    | "SUB"
                    | "SBC"
                    | "AND"
                    | "OR"
                    | "XOR"
                    | "CP"
                    | "NOP"
                    | "STOP"
                    | "DI"
                    | "SCF"
                    | "HALT"
                    | "EI"
                    | "JP"
                    | "JR"
                    | "CALL"
                    | "RET"
                    | "RETI"
                    | "RST"
                    | "INC"
                    | "DEC"
                    | "PUSH"
                    | "POP"
                    | "RLCA"
                    | "RRA"
                    | "RLA"
                    | "RRCA"
                    | "RLC"
                    | "RRC"
                    | "RL"
                    | "RR"
                    | "SLA"
                    | "SRA"
                    | "SRL"
                    | "SWAP"
                    | "BIT"
                    | "SET"
                    | "RES"
                    | "LDH"
                    | "CPL"
                    | "DAA"
                    | "CCF"
                    | "ILLEGAL_D3"
                    | "ILLEGAL_DB"
                    | "ILLEGAL_DD"
                    | "ILLEGAL_E3"
                    | "ILLEGAL_E4"
                    | "ILLEGAL_EB"
                    | "ILLEGAL_EC"
                    | "ILLEGAL_ED"
                    | "ILLEGAL_F4"
                    | "ILLEGAL_FC"
                    | "ILLEGAL_FD"
            ) {
                eprintln!("未實作指令: {} 在 PC={:04X}", mnemonic, pc_before);
            }
            // 執行指令
            crate::instructions::execute_instruction(self, mmu, opcode);

            // 返回指令的週期數
            // 條件分支指令有兩個週期值: cycles[0] = 成立, cycles[1] = 不成立
            
            if opcode.cycles.len() > 1 && !self.branch_taken {
                opcode.cycles[1] as u32
            } else {
                opcode.cycles[0] as u32
            }
        } else {
            eprintln!(
                "Unknown opcode: {:02X} at PC={:04X}",
                first_byte,
                self.pc.wrapping_sub(1)
            );
            4
        }
    }

    // 寄存器訪問方法
    pub fn get_af(&self) -> u16 {
        ((self.registers.a as u16) << 8) | (self.f() as u16)
    }

    pub fn set_af(&mut self, value: u16) {
        self.registers.a = (value >> 8) as u8;
        self.set_f(value as u8);
    }

    pub fn get_bc(&self) -> u16 {
        ((self.registers.b as u16) << 8) | (self.registers.c as u16)
    }

    pub fn set_bc(&mut self, value: u16) {
        self.registers.b = (value >> 8) as u8;
        self.registers.c = (value & 0xFF) as u8;
    }

    pub fn get_de(&self) -> u16 {
        ((self.registers.d as u16) << 8) | (self.registers.e as u16)
    }

    pub fn set_de(&mut self, value: u16) {
        self.registers.d = (value >> 8) as u8;
        self.registers.e = (value & 0xFF) as u8;
    }

    pub fn get_hl(&self) -> u16 {
        ((self.registers.h as u16) << 8) | (self.registers.l as u16)
    }

    pub fn set_hl(&mut self, value: u16) {
        self.registers.h = (value >> 8) as u8;
        self.registers.l = (value & 0xFF) as u8;
    }

    // 旗標訪問方法
    pub fn get_flag_z(&self) -> bool {
        self.flags.z == FlagState::Set
    }

    pub fn set_flag_z(&mut self, value: bool) {
        self.flags.z = if value {
            FlagState::Set
        } else {
            FlagState::Clear
        };
    }

    pub fn get_flag_n(&self) -> bool {
        self.flags.n == FlagState::Set
    }

    pub fn set_flag_n(&mut self, value: bool) {
        self.flags.n = if value {
            FlagState::Set
        } else {
            FlagState::Clear
        };
    }

    pub fn get_flag_h(&self) -> bool {
        self.flags.h == FlagState::Set
    }

    pub fn set_flag_h(&mut self, value: bool) {
        self.flags.h = if value {
            FlagState::Set
        } else {
            FlagState::Clear
        };
    }

    pub fn get_flag_c(&self) -> bool {
        self.flags.c == FlagState::Set
    }

    pub fn set_flag_c(&mut self, value: bool) {
        self.flags.c = if value {
            FlagState::Set
        } else {
            FlagState::Clear
        };
    }

    // 直接寄存器訪問 (為了向後相容)
    pub fn a(&self) -> u8 {
        self.registers.a
    }

    pub fn set_a(&mut self, value: u8) {
        self.registers.a = value;
    }

    pub fn f(&self) -> u8 {
        let mut f = 0u8;
        if self.flags.z == FlagState::Set {
            f |= 0x80;
        }
        if self.flags.n == FlagState::Set {
            f |= 0x40;
        }
        if self.flags.h == FlagState::Set {
            f |= 0x20;
        }
        if self.flags.c == FlagState::Set {
            f |= 0x10;
        }
        f
    }

    pub fn set_f(&mut self, value: u8) {
        self.flags.z = if (value & 0x80) != 0 {
            FlagState::Set
        } else {
            FlagState::Clear
        };
        self.flags.n = if (value & 0x40) != 0 {
            FlagState::Set
        } else {
            FlagState::Clear
        };
        self.flags.h = if (value & 0x20) != 0 {
            FlagState::Set
        } else {
            FlagState::Clear
        };
        self.flags.c = if (value & 0x10) != 0 {
            FlagState::Set
        } else {
            FlagState::Clear
        };
        self.registers.f = value & 0xF0;
    }

    pub fn b(&self) -> u8 {
        self.registers.b
    }

    pub fn set_b(&mut self, value: u8) {
        self.registers.b = value;
    }

    pub fn c(&self) -> u8 {
        self.registers.c
    }

    pub fn set_c(&mut self, value: u8) {
        self.registers.c = value;
    }

    pub fn d(&self) -> u8 {
        self.registers.d
    }

    pub fn set_d(&mut self, value: u8) {
        self.registers.d = value;
    }

    pub fn e(&self) -> u8 {
        self.registers.e
    }

    pub fn set_e(&mut self, value: u8) {
        self.registers.e = value;
    }

    pub fn h(&self) -> u8 {
        self.registers.h
    }

    pub fn set_h(&mut self, value: u8) {
        self.registers.h = value;
    }

    pub fn l(&self) -> u8 {
        self.registers.l
    }

    pub fn set_l(&mut self, value: u8) {
        self.registers.l = value;
    }
}

// 載入操作碼資料的函數
pub fn load_opcodes() -> std::io::Result<Opcodes> {
    let data = include_str!("../Opcodes.json");

    #[derive(Deserialize)]
    struct RawOpcodes {
        unprefixed: HashMap<String, Opcode>,
        cbprefixed: HashMap<String, Opcode>,
    }

    let raw: RawOpcodes = serde_json::from_str(data).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let mut unprefixed = Vec::with_capacity(256);
    let mut cbprefixed = Vec::with_capacity(256);

    for _ in 0..256 {
        unprefixed.push(None);
        cbprefixed.push(None);
    }

    for (key, val) in raw.unprefixed {
        if let Ok(code) = u8::from_str_radix(key.trim_start_matches("0x"), 16) {
            unprefixed[code as usize] = Some(val);
        }
    }

    for (key, val) in raw.cbprefixed {
        if let Ok(code) = u8::from_str_radix(key.trim_start_matches("0x"), 16) {
            cbprefixed[code as usize] = Some(val);
        }
    }

    Ok(Opcodes {
        unprefixed,
        cbprefixed,
    })
}
#[test]
fn test_load_opcodes() {
    load_opcodes().unwrap();
}
