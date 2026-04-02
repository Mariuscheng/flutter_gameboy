//! 跳轉和控制流指令的處理模組
//!
//! 這個模組負責處理 JP, JR, CALL, RET 等跳轉指令

use crate::cpu::Cpu;
use crate::mmu::Mmu;

/// 處理 JP (Jump) 指令
pub fn handle_jp(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    if !opcode.operands.is_empty() {
        match opcode.operands[0].name.as_str() {
            "a16" => {
                // JP a16 - 絕對跳轉 (無條件，總是 taken)
                let addr = cpu.fetch_word(mmu);
                cpu.pc = addr;
                cpu.branch_taken = true;
            }
            "HL" => {
                // JP HL - 跳轉到 HL 寄存器 (無條件，總是 taken)
                cpu.pc = cpu.get_hl();
                cpu.branch_taken = true;
            }
            _ => {
                // JP cc,a16 - 條件跳轉
                if let Some(condition) = get_condition(&opcode.operands[0].name) {
                    let addr = cpu.fetch_word(mmu);
                    if check_condition(cpu, condition) {
                        cpu.pc = addr;
                        cpu.branch_taken = true;
                    }
                }
            }
        }
    }
}

/// 處理 JR (Jump Relative) 指令
pub fn handle_jr(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    if !opcode.operands.is_empty() {
        match opcode.operands[0].name.as_str() {
            "e8" => {
                // JR e8 - 相對跳轉 (無條件，總是 taken)
                let offset = cpu.fetch_byte(mmu) as i8;
                cpu.pc = cpu.pc.wrapping_add_signed(offset as i16);
                cpu.branch_taken = true;
            }
            _ => {
                // JR cc,e8 - 條件相對跳轉
                if let Some(condition) = get_condition(&opcode.operands[0].name) {
                    let offset = cpu.fetch_byte(mmu) as i8;
                    if check_condition(cpu, condition) {
                        cpu.pc = cpu.pc.wrapping_add_signed(offset as i16);
                        cpu.branch_taken = true;
                    }
                }
            }
        }
    }
}

/// 處理 CALL 指令
pub fn handle_call(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    if !opcode.operands.is_empty() {
        match opcode.operands[0].name.as_str() {
            "a16" => {
                // CALL a16 - 呼叫子程序 (無條件，總是 taken)
                let addr = cpu.fetch_word(mmu);
                cpu.push_word(mmu, cpu.pc);
                cpu.pc = addr;
                cpu.branch_taken = true;
            }
            _ => {
                // CALL cc,a16 - 條件呼叫
                if let Some(condition) = get_condition(&opcode.operands[0].name) {
                    let addr = cpu.fetch_word(mmu);
                    if check_condition(cpu, condition) {
                        cpu.push_word(mmu, cpu.pc);
                        cpu.pc = addr;
                        cpu.branch_taken = true;
                    }
                }
            }
        }
    }
}

/// 處理 RET (Return) 指令
pub fn handle_ret(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    if opcode.operands.is_empty() {
        // RET - 返回 (無條件，總是 taken)
        cpu.pc = cpu.pop_word(&*mmu);
        cpu.branch_taken = true;
    } else {
        // RET cc - 條件返回
        if let Some(condition) = get_condition(&opcode.operands[0].name)
            && check_condition(cpu, condition) {
                cpu.pc = cpu.pop_word(&*mmu);
                cpu.branch_taken = true;
            }
    }
}

/// 處理 RETI (Return from Interrupt) 指令
pub fn handle_reti(cpu: &mut Cpu, mmu: &mut Mmu, _opcode: &crate::cpu::Opcode) {
    cpu.pc = cpu.pop_word(mmu);
    cpu.ime = crate::cpu::InterruptMasterState::Enabled; // 重新啟用中斷
    cpu.branch_taken = true;
}

/// 處理 RST (Restart) 指令
pub fn handle_rst(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    // RST 指令將 PC 推入堆疊，然後跳轉到指定位址
    if let Some(operand) = opcode.operands.first() {
        let name = operand.name.to_lowercase();
        let addr_str = name
            .trim_start_matches('$')
            .trim_start_matches("0x")
            .trim_end_matches('h');

        if let Ok(addr) = u16::from_str_radix(addr_str, 16) {
            let pc = cpu.pc;
            cpu.push_word(mmu, pc);
            cpu.pc = addr;
            cpu.branch_taken = true;
        }
    }
}

/// 處理 HALT 指令
pub fn handle_halt(cpu: &mut Cpu, mmu: &mut Mmu, _opcode: &crate::cpu::Opcode) {
    // 檢查 HALT bug 條件：IME=0 且有中斷待處理 (IE & IF != 0)
    let ie = mmu.read_byte(0xFFFF);
    let iff = mmu.read_byte(0xFF0F);
    let pending = ie & iff & 0x1F;

    if cpu.ime == crate::cpu::InterruptMasterState::Disabled && pending != 0 {
        // HALT bug: 下一條指令的第一個 byte 會被讀取兩次
        // 這會導致 PC 不遞增，指令被重複執行
        cpu.halt_bug = true;
        // 不進入 Halted 狀態，繼續執行
    } else {
        cpu.state = crate::cpu::CpuState::Halted;
    }
}

/// 處理 EI (Enable Interrupts) 指令
pub fn handle_ei(cpu: &mut Cpu, _mmu: &mut Mmu, _opcode: &crate::cpu::Opcode) {
    // EI 不會立即生效，而是在下一個指令之後啟用 IME
    cpu.ime = crate::cpu::InterruptMasterState::Pending;
}

/// 從操作數名稱提取條件
fn get_condition(operand: &str) -> Option<&str> {
    match operand {
        "NZ" => Some("NZ"),
        "Z" => Some("Z"),
        "NC" => Some("NC"),
        "C" => Some("C"),
        _ => None,
    }
}

/// 檢查條件是否滿足
fn check_condition(cpu: &Cpu, condition: &str) -> bool {
    match condition {
        "NZ" => !cpu.get_flag_z(),
        "Z" => cpu.get_flag_z(),
        "NC" => !cpu.get_flag_c(),
        "C" => cpu.get_flag_c(),
        _ => false,
    }
}
