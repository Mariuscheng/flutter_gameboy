//! 雜項指令的處理模組
//!
//! 這個模組負責處理 LDH, CPL, DAA, RLA, RRCA 等雜項指令

use crate::cpu::Cpu;
use crate::mmu::Mmu;

/// 處理 LDH (Load High) 指令
pub fn handle_ldh(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    if opcode.operands.len() >= 2 {
        let op0 = &opcode.operands[0];
        let op1 = &opcode.operands[1];
        match (
            op0.name.as_str(),
            op0.immediate,
            op1.name.as_str(),
            op1.immediate,
        ) {
            ("A", _, "a8", Some(false)) => {
                // LDH A, (a8) - 從高位地址載入到 A
                let offset = cpu.fetch_byte(mmu);
                let addr = 0xFF00 | (offset as u16);
                let value = mmu.read_byte(addr);
                cpu.set_a(value);
            }
            ("a8", Some(false), "A", _) => {
                // LDH (a8), A - 從 A 存儲到高位地址
                let addr = 0xFF00 | (cpu.fetch_byte(mmu) as u16);
                mmu.write_byte(addr, cpu.a());
            }
            ("A", _, "C", Some(false)) => {
                // LDH A, (C) - 從 C 寄存器指定的高位地址載入到 A
                let addr = 0xFF00 | (cpu.c() as u16);
                cpu.set_a(mmu.read_byte(addr));
            }
            ("C", Some(false), "A", _) => {
                // LDH (C), A - 從 A 存儲到 C 寄存器指定的高位地址
                let addr = 0xFF00 | (cpu.c() as u16);
                mmu.write_byte(addr, cpu.a());
            }
            _ => {
                // LDH 變體未實作时保持沉默
            }
        }
    }
}

/// 處理 CPL (Complement) 指令
pub fn handle_cpl(cpu: &mut Cpu, _opcode: &crate::cpu::Opcode) {
    cpu.set_a(!cpu.a());
    cpu.set_flag_n(true);
    cpu.set_flag_h(true);
}

/// 處理 DAA (Decimal Adjust Accumulator) 指令
pub fn handle_daa(cpu: &mut Cpu, _opcode: &crate::cpu::Opcode) {
    let mut a = cpu.a() as u16;

    if !cpu.get_flag_n() {
        if cpu.get_flag_h() || (a & 0x0F) > 0x09 {
            a += 0x06;
        }
        if cpu.get_flag_c() || a > 0x9F {
            a += 0x60;
        }
    } else {
        if cpu.get_flag_h() {
            a = a.wrapping_sub(0x06) & 0xFF;
        }
        if cpu.get_flag_c() {
            a = a.wrapping_sub(0x60) & 0xFF;
        }
    }

    cpu.set_flag_h(false);
    cpu.set_flag_z((a as u8) == 0);

    if (a & 0x100) != 0 {
        cpu.set_flag_c(true);
    }

    cpu.set_a(a as u8);
}

/// 處理 RLA (Rotate Left Accumulator) 指令
pub fn handle_rla(cpu: &mut Cpu, _opcode: &crate::cpu::Opcode) {
    let carry = cpu.get_flag_c() as u8;
    let new_carry = (cpu.a() & 0x80) != 0;

    cpu.set_a((cpu.a() << 1) | carry);
    cpu.set_flag_z(false);
    cpu.set_flag_n(false);
    cpu.set_flag_h(false);
    cpu.set_flag_c(new_carry);
}

/// 處理 RRCA (Rotate Right Circular Accumulator) 指令
pub fn handle_rrca(cpu: &mut Cpu, _opcode: &crate::cpu::Opcode) {
    let carry = (cpu.a() & 0x01) != 0;
    cpu.set_a((cpu.a() >> 1) | ((carry as u8) << 7));

    cpu.set_flag_z(false);
    cpu.set_flag_n(false);
    cpu.set_flag_h(false);
    cpu.set_flag_c(carry);
}

/// 處理 CCF (Complement Carry Flag) 指令
pub fn handle_ccf(cpu: &mut Cpu, _opcode: &crate::cpu::Opcode) {
    cpu.set_flag_n(false);
    cpu.set_flag_h(false);
    cpu.set_flag_c(!cpu.get_flag_c());
}
