//! 堆疊指令的處理模組
//!
//! 這個模組負責處理 PUSH, POP 等堆疊操作指令

use crate::cpu::Cpu;
use crate::mmu::Mmu;

/// 處理 PUSH 指令
pub fn handle_push(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    if !opcode.operands.is_empty() {
        match opcode.operands[0].name.as_str() {
            "AF" => {
                cpu.push_word(mmu, cpu.get_af());
            }
            "BC" => {
                cpu.push_word(mmu, cpu.get_bc());
            }
            "DE" => {
                cpu.push_word(mmu, cpu.get_de());
            }
            "HL" => {
                cpu.push_word(mmu, cpu.get_hl());
            }
            _ => {}
        }
    }
}

/// 處理 POP 指令
pub fn handle_pop(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    if !opcode.operands.is_empty() {
        let val = cpu.pop_word(&*mmu);
        match opcode.operands[0].name.as_str() {
            "AF" => {
                cpu.set_af(val);
            }
            "BC" => {
                cpu.set_bc(val);
            }
            "DE" => {
                cpu.set_de(val);
            }
            "HL" => {
                cpu.set_hl(val);
            }
            _ => {}
        }
    }
}
