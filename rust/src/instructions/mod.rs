//! 指令處理模組
//!
//! 這個模組整合了所有指令類型的處理器

pub mod arithmetic;
pub mod control;
pub mod inc_dec;
pub mod jump;
pub mod ld;
pub mod misc;
pub mod rotate;
pub mod stack;

use crate::cpu::Cpu;
use crate::mmu::Mmu;

/// 主要的指令處理器
pub fn execute_instruction(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    match opcode.mnemonic.as_str() {
        // 載入指令
        "LD" => ld::handle_ld(cpu, mmu, opcode),

        // 算術指令
        "ADD" => arithmetic::handle_add(cpu, mmu, opcode),
        "ADC" => arithmetic::handle_adc(cpu, mmu, opcode),
        "SUB" => arithmetic::handle_sub(cpu, mmu, opcode),
        "SBC" => arithmetic::handle_sbc(cpu, mmu, opcode),
        "AND" => arithmetic::handle_and(cpu, mmu, opcode),
        "OR" => arithmetic::handle_or(cpu, mmu, opcode),
        "XOR" => arithmetic::handle_xor(cpu, mmu, opcode),
        "CP" => arithmetic::handle_cp(cpu, mmu, opcode),

        // 控制指令
        "NOP" => control::handle_nop(cpu, opcode),
        "STOP" => control::handle_stop(cpu, mmu, opcode),
        "DI" => control::handle_di(cpu, mmu, opcode),
        "SCF" => control::handle_scf(cpu, mmu, opcode),
        "HALT" => jump::handle_halt(cpu, mmu, opcode),
        "EI" => jump::handle_ei(cpu, mmu, opcode),

        // 跳轉指令
        "JP" => jump::handle_jp(cpu, mmu, opcode),
        "JR" => jump::handle_jr(cpu, mmu, opcode),
        "CALL" => jump::handle_call(cpu, mmu, opcode),
        "RET" => jump::handle_ret(cpu, mmu, opcode),
        "RETI" => jump::handle_reti(cpu, mmu, opcode),
        "RST" => jump::handle_rst(cpu, mmu, opcode),

        // 增減指令
        "INC" => inc_dec::handle_inc(cpu, mmu, opcode),
        "DEC" => inc_dec::handle_dec(cpu, mmu, opcode),

        // 堆疊指令
        "PUSH" => stack::handle_push(cpu, mmu, opcode),
        "POP" => stack::handle_pop(cpu, mmu, opcode),

        // 旋轉指令
        "RLCA" => rotate::handle_rlca(cpu, opcode),
        "RRA" => rotate::handle_rra(cpu, opcode),
        "RLA" => misc::handle_rla(cpu, opcode),
        "RRCA" => misc::handle_rrca(cpu, opcode),
        "RLC" => rotate::handle_rlc(cpu, mmu, opcode),
        "RRC" => rotate::handle_rrc(cpu, mmu, opcode),
        "RL" => rotate::handle_rl(cpu, mmu, opcode),
        "RR" => rotate::handle_rr(cpu, mmu, opcode),
        "SLA" => rotate::handle_sla(cpu, mmu, opcode),
        "SRA" => rotate::handle_sra(cpu, mmu, opcode),
        "SRL" => rotate::handle_srl(cpu, mmu, opcode),
        "SWAP" => rotate::handle_swap(cpu, mmu, opcode),

        // BIT 指令
        "BIT" => rotate::handle_bit(cpu, mmu, opcode),

        // SET 和 RES 指令
        "SET" => rotate::handle_set(cpu, mmu, opcode),
        "RES" => rotate::handle_res(cpu, mmu, opcode),

        // 雜項指令
        "LDH" => misc::handle_ldh(cpu, mmu, opcode),
        "CPL" => misc::handle_cpl(cpu, opcode),
        "DAA" => misc::handle_daa(cpu, opcode),
        "CCF" => misc::handle_ccf(cpu, opcode),

        // ILLEGAL opcodes that are declared in the opcode table but intentionally do nothing
        "ILLEGAL_D3" | "ILLEGAL_DB" | "ILLEGAL_DD" | "ILLEGAL_E3" | "ILLEGAL_E4" | "ILLEGAL_EB"
        | "ILLEGAL_EC" | "ILLEGAL_ED" | "ILLEGAL_F4" | "ILLEGAL_FC" | "ILLEGAL_FD" => {
            // Treated as defined but inert opcodes — consume cycles (no state change).
            // Keep explicit handler to avoid falling into the generic "尚未實作" message.
        }

        // 未實現的指令
        _ => {
            // 指令未實作时保持沉默
        }
    }
}
