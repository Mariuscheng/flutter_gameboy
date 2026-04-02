//! 算術指令的處理模組
//!
//! 包含 ADD, ADC, SUB, SBC, AND, OR, XOR, CP 等指令

use crate::cpu::Cpu;
use crate::mmu::Mmu;

/// 處理 ADD 指令
pub fn handle_add(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    if opcode.operands.len() >= 2 {
        let original_a = cpu.a();
        // Handle 16-bit form where HL is the destination: `ADD HL, rr`
        if opcode.operands[0].name == "HL" {
            let hl = cpu.get_hl();
            let val = match opcode.operands[1].name.as_str() {
                "BC" => cpu.get_bc(),
                "DE" => cpu.get_de(),
                "HL" => cpu.get_hl(),
                "SP" => cpu.sp,
                _ => 0,
            };
            let result = hl as u32 + val as u32;
            cpu.set_hl(result as u16);
            cpu.set_flag_n(false);
            cpu.set_flag_h(((hl & 0x0FFF) + (val & 0x0FFF)) > 0x0FFF);
            cpu.set_flag_c(result > 0xFFFF);
            return;
        }

        let val = match opcode.operands[1].name.as_str() {
            "A" => cpu.a(),
            "B" => cpu.b(),
            "C" => cpu.c(),
            "D" => cpu.d(),
            "E" => cpu.e(),
            "H" => cpu.h(),
            "L" => cpu.l(),
            "HL" => mmu.read_byte(cpu.get_hl()),
            "n8" | "e8" | "r8" => cpu.fetch_byte(mmu),
            _ => 0,
        };

        if opcode.operands[0].name == "SP" {
            let offset = val as i8 as i32;
            let result = cpu.sp as i32 + offset;
            cpu.set_flag_z(false);
            cpu.set_flag_n(false);
            cpu.set_flag_h(((cpu.sp & 0x0F) as i32 + (val & 0x0F) as i32) > 0x0F);
            cpu.set_flag_c(((cpu.sp & 0xFF) as i32 + val as i32) > 0xFF);
            cpu.sp = (result & 0xFFFF) as u16;
            return;
        }

        let result = original_a as u16 + val as u16;
        cpu.set_a(result as u8);
        cpu.set_flag_z(cpu.a() == 0);
        cpu.set_flag_n(false);
        cpu.set_flag_h(((original_a & 0x0F) + (val & 0x0F)) > 0x0F);
        cpu.set_flag_c(result > 0xFF);
    }
}

/// 處理 ADC (帶進位加法) 指令
pub fn handle_adc(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    if opcode.operands.len() >= 2 {
        let original_a = cpu.a();
        let carry = cpu.get_flag_c() as u8;
        let val = match opcode.operands[1].name.as_str() {
            "A" => cpu.a(),
            "B" => cpu.b(),
            "C" => cpu.c(),
            "D" => cpu.d(),
            "E" => cpu.e(),
            "H" => cpu.h(),
            "L" => cpu.l(),
            "HL" => mmu.read_byte(cpu.get_hl()),
            "n8" | "e8" | "r8" => cpu.fetch_byte(mmu),
            _ => 0,
        };

        let result = original_a as u16 + val as u16 + carry as u16;
        cpu.set_a(result as u8);
        cpu.set_flag_z(cpu.a() == 0);
        cpu.set_flag_n(false);
        cpu.set_flag_h(((original_a & 0x0F) + (val & 0x0F) + carry) > 0x0F);
        cpu.set_flag_c(result > 0xFF);
    }
}

/// 處理 SUB (減法) 指令
pub fn handle_sub(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    if opcode.operands.len() >= 2 {
        let original_a = cpu.a();
        let val = match opcode.operands[1].name.as_str() {
            "A" => cpu.a(),
            "B" => cpu.b(),
            "C" => cpu.c(),
            "D" => cpu.d(),
            "E" => cpu.e(),
            "H" => cpu.h(),
            "L" => cpu.l(),
            "HL" => mmu.read_byte(cpu.get_hl()),
            "n8" | "e8" | "r8" => cpu.fetch_byte(mmu),
            _ => 0,
        };

        cpu.set_a(original_a.wrapping_sub(val));
        cpu.set_flag_z(cpu.a() == 0);
        cpu.set_flag_n(true);
        cpu.set_flag_h((original_a & 0x0F) < (val & 0x0F));
        cpu.set_flag_c(original_a < val);
    } else if opcode.operands.len() == 1 {
        // 某些 SUB 指令只有一個 operand (SUB r8)
        let original_a = cpu.a();
        let val = match opcode.operands[0].name.as_str() {
            "A" => cpu.a(),
            "B" => cpu.b(),
            "C" => cpu.c(),
            "D" => cpu.d(),
            "E" => cpu.e(),
            "H" => cpu.h(),
            "L" => cpu.l(),
            "HL" => mmu.read_byte(cpu.get_hl()),
            "n8" | "e8" | "r8" => cpu.fetch_byte(mmu),
            _ => 0,
        };

        cpu.set_a(original_a.wrapping_sub(val));
        cpu.set_flag_z(cpu.a() == 0);
        cpu.set_flag_n(true);
        cpu.set_flag_h((original_a & 0x0F) < (val & 0x0F));
        cpu.set_flag_c(original_a < val);
    }
}

/// 處理 SBC (帶借位減法) 指令
pub fn handle_sbc(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    if opcode.operands.len() >= 2 {
        let original_a = cpu.a();
        let carry = cpu.get_flag_c() as u8;
        let val = match opcode.operands[1].name.as_str() {
            "A" => cpu.a(),
            "B" => cpu.b(),
            "C" => cpu.c(),
            "D" => cpu.d(),
            "E" => cpu.e(),
            "H" => cpu.h(),
            "L" => cpu.l(),
            "HL" => mmu.read_byte(cpu.get_hl()),
            "n8" | "e8" | "r8" => cpu.fetch_byte(mmu),
            _ => 0,
        };

        let result = original_a as i16 - val as i16 - carry as i16;
        cpu.set_a(result as u8);
        cpu.set_flag_z(cpu.a() == 0);
        cpu.set_flag_n(true);
        cpu.set_flag_h(((original_a & 0x0F) as i16 - (val & 0x0F) as i16 - carry as i16) < 0);
        cpu.set_flag_c(result < 0);
    }
}

/// 處理 AND 指令
pub fn handle_and(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    if opcode.operands.len() >= 2 {
        let val = match opcode.operands[1].name.as_str() {
            "A" => cpu.a(),
            "B" => cpu.b(),
            "C" => cpu.c(),
            "D" => cpu.d(),
            "E" => cpu.e(),
            "H" => cpu.h(),
            "L" => cpu.l(),
            "HL" => mmu.read_byte(cpu.get_hl()),
            "n8" | "e8" | "r8" => cpu.fetch_byte(mmu),
            _ => 0,
        };

        cpu.set_a(cpu.a() & val);
        cpu.set_flag_z(cpu.a() == 0);
        cpu.set_flag_n(false);
        cpu.set_flag_h(true);
        cpu.set_flag_c(false);
    }
}

/// 處理 OR 指令
pub fn handle_or(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    if opcode.operands.len() >= 2 {
        let val = match opcode.operands[1].name.as_str() {
            "A" => cpu.a(),
            "B" => cpu.b(),
            "C" => cpu.c(),
            "D" => cpu.d(),
            "E" => cpu.e(),
            "H" => cpu.h(),
            "L" => cpu.l(),
            "HL" => mmu.read_byte(cpu.get_hl()),
            "n8" | "e8" | "r8" => cpu.fetch_byte(mmu),
            _ => 0,
        };

        cpu.set_a(cpu.a() | val);
        cpu.set_flag_z(cpu.a() == 0);
        cpu.set_flag_n(false);
        cpu.set_flag_h(false);
        cpu.set_flag_c(false);
    }
}

/// 處理 XOR 指令
pub fn handle_xor(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    if opcode.operands.len() >= 2 {
        let val = match opcode.operands[1].name.as_str() {
            "A" => cpu.a(),
            "B" => cpu.b(),
            "C" => cpu.c(),
            "D" => cpu.d(),
            "E" => cpu.e(),
            "H" => cpu.h(),
            "L" => cpu.l(),
            "HL" => mmu.read_byte(cpu.get_hl()),
            "n8" | "e8" | "r8" => cpu.fetch_byte(mmu),
            _ => 0,
        };

        cpu.set_a(cpu.a() ^ val);
        cpu.set_flag_z(cpu.a() == 0);
        cpu.set_flag_n(false);
        cpu.set_flag_h(false);
        cpu.set_flag_c(false);
    }
}

/// 處理 CP (比較) 指令
pub fn handle_cp(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    if opcode.operands.len() >= 2 {
        let original_a = cpu.a();
        let val = match opcode.operands[1].name.as_str() {
            "A" => cpu.a(),
            "B" => cpu.b(),
            "C" => cpu.c(),
            "D" => cpu.d(),
            "E" => cpu.e(),
            "H" => cpu.h(),
            "L" => cpu.l(),
            "HL" => mmu.read_byte(cpu.get_hl()),
            "n8" | "e8" | "r8" => cpu.fetch_byte(mmu),
            _ => 0,
        };

        let result = original_a.wrapping_sub(val);
        cpu.set_flag_z(result == 0);
        cpu.set_flag_n(true);
        cpu.set_flag_h((original_a & 0x0F) < (val & 0x0F));
        cpu.set_flag_c(original_a < val);
        // CP 不修改 A 寄存器
    }
}
