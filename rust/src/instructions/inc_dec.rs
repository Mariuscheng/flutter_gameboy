//! 增減指令的處理模組
//!
//! 這個模組負責處理 INC, DEC 等增減指令

use crate::cpu::Cpu;
use crate::mmu::Mmu;

/// 處理 INC (Increment) 指令
pub fn handle_inc(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    if !opcode.operands.is_empty() {
        let op = &opcode.operands[0];
        let name = op.name.as_str();
        let imm = op.immediate.unwrap_or(true);

        match (name, imm) {
            ("A", true) => {
                let val = cpu.a();
                cpu.set_a(val.wrapping_add(1));
                cpu.set_flag_z(cpu.a() == 0);
                cpu.set_flag_n(false);
                cpu.set_flag_h((val & 0x0F) == 0x0F);
            }
            ("B", true) => {
                let val = cpu.b();
                cpu.set_b(val.wrapping_add(1));
                cpu.set_flag_z(cpu.b() == 0);
                cpu.set_flag_n(false);
                cpu.set_flag_h((val & 0x0F) == 0x0F);
            }
            ("C", true) => {
                let val = cpu.c();
                cpu.set_c(val.wrapping_add(1));
                cpu.set_flag_z(cpu.c() == 0);
                cpu.set_flag_n(false);
                cpu.set_flag_h((val & 0x0F) == 0x0F);
            }
            ("D", true) => {
                let val = cpu.d();
                cpu.set_d(val.wrapping_add(1));
                cpu.set_flag_z(cpu.d() == 0);
                cpu.set_flag_n(false);
                cpu.set_flag_h((val & 0x0F) == 0x0F);
            }
            ("E", true) => {
                let val = cpu.e();
                cpu.set_e(val.wrapping_add(1));
                cpu.set_flag_z(cpu.e() == 0);
                cpu.set_flag_n(false);
                cpu.set_flag_h((val & 0x0F) == 0x0F);
            }
            ("H", true) => {
                let val = cpu.h();
                cpu.set_h(val.wrapping_add(1));
                cpu.set_flag_z(cpu.h() == 0);
                cpu.set_flag_n(false);
                cpu.set_flag_h((val & 0x0F) == 0x0F);
            }
            ("L", true) => {
                let val = cpu.l();
                cpu.set_l(val.wrapping_add(1));
                cpu.set_flag_z(cpu.l() == 0);
                cpu.set_flag_n(false);
                cpu.set_flag_h((val & 0x0F) == 0x0F);
            }
            ("BC", true) => {
                let val = cpu.get_bc();
                cpu.set_bc(val.wrapping_add(1));
            }
            ("DE", true) => {
                let val = cpu.get_de();
                cpu.set_de(val.wrapping_add(1));
            }
            ("HL", true) => {
                let val = cpu.get_hl();
                cpu.set_hl(val.wrapping_add(1));
            }
            ("SP", true) => {
                let val = cpu.sp;
                cpu.sp = val.wrapping_add(1);
            }
            ("HL", false) => {
                let addr = cpu.get_hl();
                let val = mmu.read_byte(addr);
                let new_val = val.wrapping_add(1);
                mmu.write_byte(addr, new_val);
                cpu.set_flag_z(new_val == 0);
                cpu.set_flag_n(false);
                cpu.set_flag_h((val & 0x0F) == 0x0F);
            }
            _ => {
                // 尚未實作時保持沉默
            }
        }
    }
}

/// 處理 DEC (Decrement) 指令
pub fn handle_dec(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    if !opcode.operands.is_empty() {
        let op = &opcode.operands[0];
        let name = op.name.as_str();
        let imm = op.immediate.unwrap_or(true);

        match (name, imm) {
            ("A", true) => {
                let val = cpu.a();
                cpu.set_a(val.wrapping_sub(1));
                cpu.set_flag_z(cpu.a() == 0);
                cpu.set_flag_n(true);
                cpu.set_flag_h((val & 0x0F) == 0x00);
            }
            ("B", true) => {
                let val = cpu.b();
                cpu.set_b(val.wrapping_sub(1));
                cpu.set_flag_z(cpu.b() == 0);
                cpu.set_flag_n(true);
                cpu.set_flag_h((val & 0x0F) == 0x00);
            }
            ("C", true) => {
                let val = cpu.c();
                cpu.set_c(val.wrapping_sub(1));
                cpu.set_flag_z(cpu.c() == 0);
                cpu.set_flag_n(true);
                cpu.set_flag_h((val & 0x0F) == 0x00);
            }
            ("D", true) => {
                let val = cpu.d();
                cpu.set_d(val.wrapping_sub(1));
                cpu.set_flag_z(cpu.d() == 0);
                cpu.set_flag_n(true);
                cpu.set_flag_h((val & 0x0F) == 0x00);
            }
            ("E", true) => {
                let val = cpu.e();
                cpu.set_e(val.wrapping_sub(1));
                cpu.set_flag_z(cpu.e() == 0);
                cpu.set_flag_n(true);
                cpu.set_flag_h((val & 0x0F) == 0x00);
            }
            ("H", true) => {
                let val = cpu.h();
                cpu.set_h(val.wrapping_sub(1));
                cpu.set_flag_z(cpu.h() == 0);
                cpu.set_flag_n(true);
                cpu.set_flag_h((val & 0x0F) == 0x00);
            }
            ("L", true) => {
                let val = cpu.l();
                cpu.set_l(val.wrapping_sub(1));
                cpu.set_flag_z(cpu.l() == 0);
                cpu.set_flag_n(true);
                cpu.set_flag_h((val & 0x0F) == 0x00);
            }
            ("BC", true) => {
                let val = cpu.get_bc();
                cpu.set_bc(val.wrapping_sub(1));
            }
            ("DE", true) => {
                let val = cpu.get_de();
                cpu.set_de(val.wrapping_sub(1));
            }
            ("HL", true) => {
                let val = cpu.get_hl();
                cpu.set_hl(val.wrapping_sub(1));
            }
            ("SP", true) => {
                let val = cpu.sp;
                cpu.sp = val.wrapping_sub(1);
            }
            ("HL", false) => {
                let addr = cpu.get_hl();
                let val = mmu.read_byte(addr);
                let new_val = val.wrapping_sub(1);
                mmu.write_byte(addr, new_val);
                cpu.set_flag_z(new_val == 0);
                cpu.set_flag_n(true);
                cpu.set_flag_h((val & 0x0F) == 0x00);
            }
            _ => {
                // 尚未實作時保持沉默
            }
        }
    }
}
