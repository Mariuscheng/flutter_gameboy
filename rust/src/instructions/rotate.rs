//! 旋轉和移位指令的處理模組
//!
//! 包含 RLCA, RLA, RRCA, RRA 等旋轉指令

use crate::cpu::Cpu;
use crate::mmu::Mmu;

/// 處理 RLCA 指令 (向左旋轉累加器)
pub fn handle_rlca(cpu: &mut Cpu, _opcode: &crate::cpu::Opcode) {
    let bit7 = cpu.registers.a >> 7;
    cpu.registers.a = (cpu.registers.a << 1) | bit7;
    cpu.flags.z = crate::cpu::FlagState::Clear;
    cpu.flags.n = crate::cpu::FlagState::Clear;
    cpu.flags.h = crate::cpu::FlagState::Clear;
    cpu.flags.c = if bit7 == 1 {
        crate::cpu::FlagState::Set
    } else {
        crate::cpu::FlagState::Clear
    };
}

/// 處理 RRA 指令 (向右旋轉累位，帶進位)
pub fn handle_rra(cpu: &mut Cpu, _opcode: &crate::cpu::Opcode) {
    let carry_in = if cpu.flags.c == crate::cpu::FlagState::Set {
        1
    } else {
        0
    };
    let bit0 = cpu.registers.a & 0x01;
    cpu.registers.a = (cpu.registers.a >> 1) | (carry_in << 7);
    cpu.flags.z = crate::cpu::FlagState::Clear;
    cpu.flags.n = crate::cpu::FlagState::Clear;
    cpu.flags.h = crate::cpu::FlagState::Clear;
    cpu.flags.c = if bit0 == 1 {
        crate::cpu::FlagState::Set
    } else {
        crate::cpu::FlagState::Clear
    };
}

/// 處理 BIT 指令 (測試位元)
pub fn handle_bit(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    // 從操作數中提取位元位置和暫存器
    if opcode.operands.len() >= 2 {
        // 第一個操作數是位元位置
        let bit_pos = match opcode.operands[0].name.as_str() {
            "0" => 0,
            "1" => 1,
            "2" => 2,
            "3" => 3,
            "4" => 4,
            "5" => 5,
            "6" => 6,
            "7" => 7,
            _ => 0, // 預設值
        };

        // 第二個操作數是暫存器或 (HL)
        let reg_name = opcode.operands[1].name.as_str();
        let is_immediate = opcode.operands[1].immediate;

        let value = match (reg_name, is_immediate) {
            ("B", Some(true)) => cpu.registers.b,
            ("C", Some(true)) => cpu.registers.c,
            ("D", Some(true)) => cpu.registers.d,
            ("E", Some(true)) => cpu.registers.e,
            ("H", Some(true)) => cpu.registers.h,
            ("L", Some(true)) => cpu.registers.l,
            ("HL", Some(false)) => mmu.read_byte(cpu.get_hl()), // (HL)
            ("A", Some(true)) => cpu.registers.a,
            _ => 0,
        };

        // 測試位元
        let bit_set = (value & (1 << bit_pos)) != 0;

        // 設置旗標
        cpu.flags.z = if !bit_set {
            crate::cpu::FlagState::Set
        } else {
            crate::cpu::FlagState::Clear
        }; // Z 設為 true 如果位元為 0
        cpu.flags.n = crate::cpu::FlagState::Clear;
        cpu.flags.h = crate::cpu::FlagState::Set;
        // C 旗標不變
    }
}

/// 處理 SET 指令 (設定位元)
pub fn handle_set(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    // 從操作數中提取位元位置和暫存器
    if opcode.operands.len() >= 2 {
        // 第一個操作數是位元位置
        let bit_pos = match opcode.operands[0].name.as_str() {
            "0" => 0,
            "1" => 1,
            "2" => 2,
            "3" => 3,
            "4" => 4,
            "5" => 5,
            "6" => 6,
            "7" => 7,
            _ => 0, // 預設值
        };

        // 第二個操作數是暫存器
        let reg_name = opcode.operands[1].name.as_str();
        let is_immediate = opcode.operands[1].immediate;

        match (reg_name, is_immediate) {
            ("B", Some(true)) => cpu.registers.b |= 1 << bit_pos,
            ("C", Some(true)) => cpu.registers.c |= 1 << bit_pos,
            ("D", Some(true)) => cpu.registers.d |= 1 << bit_pos,
            ("E", Some(true)) => cpu.registers.e |= 1 << bit_pos,
            ("H", Some(true)) => cpu.registers.h |= 1 << bit_pos,
            ("L", Some(true)) => cpu.registers.l |= 1 << bit_pos,
            ("HL", Some(false)) => {
                let addr = cpu.get_hl();
                let mut value = mmu.read_byte(addr);
                value |= 1 << bit_pos;
                mmu.write_byte(addr, value);
            }
            ("A", Some(true)) => cpu.registers.a |= 1 << bit_pos,
            _ => { /* 尚未實作時保持沉默 */ }
        };

        // SET 指令不影響旗標
    }
}

/// 處理 RES 指令 (清除位元)
pub fn handle_res(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    // 從操作數中提取位元位置和暫存器
    if opcode.operands.len() >= 2 {
        // 第一個操作數是位元位置
        let bit_pos = match opcode.operands[0].name.as_str() {
            "0" => 0,
            "1" => 1,
            "2" => 2,
            "3" => 3,
            "4" => 4,
            "5" => 5,
            "6" => 6,
            "7" => 7,
            _ => 0, // 預設值
        };

        // 第二個操作數是暫存器
        let reg_name = opcode.operands[1].name.as_str();
        let is_immediate = opcode.operands[1].immediate;

        match (reg_name, is_immediate) {
            ("B", Some(true)) => cpu.registers.b &= !(1 << bit_pos),
            ("C", Some(true)) => cpu.registers.c &= !(1 << bit_pos),
            ("D", Some(true)) => cpu.registers.d &= !(1 << bit_pos),
            ("E", Some(true)) => cpu.registers.e &= !(1 << bit_pos),
            ("H", Some(true)) => cpu.registers.h &= !(1 << bit_pos),
            ("L", Some(true)) => cpu.registers.l &= !(1 << bit_pos),
            ("HL", Some(false)) => {
                let addr = cpu.get_hl();
                let mut value = mmu.read_byte(addr);
                value &= !(1 << bit_pos);
                mmu.write_byte(addr, value);
            }
            ("A", Some(true)) => cpu.registers.a &= !(1 << bit_pos),
            _ => { /* 尚未實作時保持沉默 */ }
        };

        // RES 指令不影響旗標
    }
}

/// 輔助函數：獲取操作數的值
fn get_operand_value(cpu: &Cpu, mmu: &Mmu, name: &str, immediate: Option<bool>) -> u8 {
    match (name, immediate) {
        ("A", Some(true)) => cpu.registers.a,
        ("B", Some(true)) => cpu.registers.b,
        ("C", Some(true)) => cpu.registers.c,
        ("D", Some(true)) => cpu.registers.d,
        ("E", Some(true)) => cpu.registers.e,
        ("H", Some(true)) => cpu.registers.h,
        ("L", Some(true)) => cpu.registers.l,
        ("HL", Some(false)) => mmu.read_byte(cpu.get_hl()),
        _ => 0,
    }
}

/// 輔助函數：設置操作數的值
fn set_operand_value(cpu: &mut Cpu, mmu: &mut Mmu, name: &str, immediate: Option<bool>, value: u8) {
    match (name, immediate) {
        ("A", Some(true)) => cpu.registers.a = value,
        ("B", Some(true)) => cpu.registers.b = value,
        ("C", Some(true)) => cpu.registers.c = value,
        ("D", Some(true)) => cpu.registers.d = value,
        ("E", Some(true)) => cpu.registers.e = value,
        ("H", Some(true)) => cpu.registers.h = value,
        ("L", Some(true)) => cpu.registers.l = value,
        ("HL", Some(false)) => mmu.write_byte(cpu.get_hl(), value),
        _ => {}
    }
}

pub fn handle_rlc(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    let name = &opcode.operands[0].name;
    let imm = opcode.operands[0].immediate;
    let val = get_operand_value(cpu, mmu, name, imm);
    let bit7 = val >> 7;
    let res = (val << 1) | bit7;
    set_operand_value(cpu, mmu, name, imm, res);
    cpu.set_flag_z(res == 0);
    cpu.set_flag_n(false);
    cpu.set_flag_h(false);
    cpu.set_flag_c(bit7 == 1);
}

pub fn handle_rrc(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    let name = &opcode.operands[0].name;
    let imm = opcode.operands[0].immediate;
    let val = get_operand_value(cpu, mmu, name, imm);
    let bit0 = val & 0x01;
    let res = (val >> 1) | (bit0 << 7);
    set_operand_value(cpu, mmu, name, imm, res);
    cpu.set_flag_z(res == 0);
    cpu.set_flag_n(false);
    cpu.set_flag_h(false);
    cpu.set_flag_c(bit0 == 1);
}

pub fn handle_rl(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    let name = &opcode.operands[0].name;
    let imm = opcode.operands[0].immediate;
    let val = get_operand_value(cpu, mmu, name, imm);
    let carry = if cpu.flags.c == crate::cpu::FlagState::Set {
        1
    } else {
        0
    };
    let bit7 = val >> 7;
    let res = (val << 1) | carry;
    set_operand_value(cpu, mmu, name, imm, res);
    cpu.set_flag_z(res == 0);
    cpu.set_flag_n(false);
    cpu.set_flag_h(false);
    cpu.set_flag_c(bit7 == 1);
}

pub fn handle_rr(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    let name = &opcode.operands[0].name;
    let imm = opcode.operands[0].immediate;
    let val = get_operand_value(cpu, mmu, name, imm);
    let carry = if cpu.flags.c == crate::cpu::FlagState::Set {
        1
    } else {
        0
    };
    let bit0 = val & 0x01;
    let res = (val >> 1) | (carry << 7);
    set_operand_value(cpu, mmu, name, imm, res);
    cpu.set_flag_z(res == 0);
    cpu.set_flag_n(false);
    cpu.set_flag_h(false);
    cpu.set_flag_c(bit0 == 1);
}

pub fn handle_sla(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    let name = &opcode.operands[0].name;
    let imm = opcode.operands[0].immediate;
    let val = get_operand_value(cpu, mmu, name, imm);
    let bit7 = val >> 7;
    let res = val << 1;
    set_operand_value(cpu, mmu, name, imm, res);
    cpu.set_flag_z(res == 0);
    cpu.set_flag_n(false);
    cpu.set_flag_h(false);
    cpu.set_flag_c(bit7 == 1);
}

pub fn handle_sra(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    let name = &opcode.operands[0].name;
    let imm = opcode.operands[0].immediate;
    let val = get_operand_value(cpu, mmu, name, imm);
    let bit0 = val & 0x01;
    let bit7 = val & 0x80;
    let res = (val >> 1) | bit7;
    set_operand_value(cpu, mmu, name, imm, res);
    cpu.set_flag_z(res == 0);
    cpu.set_flag_n(false);
    cpu.set_flag_h(false);
    cpu.set_flag_c(bit0 == 1);
}

pub fn handle_srl(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    let name = &opcode.operands[0].name;
    let imm = opcode.operands[0].immediate;
    let val = get_operand_value(cpu, mmu, name, imm);
    let bit0 = val & 0x01;
    let res = val >> 1;
    set_operand_value(cpu, mmu, name, imm, res);
    cpu.set_flag_z(res == 0);
    cpu.set_flag_n(false);
    cpu.set_flag_h(false);
    cpu.set_flag_c(bit0 == 1);
}

pub fn handle_swap(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    let name = &opcode.operands[0].name;
    let imm = opcode.operands[0].immediate;
    let val = get_operand_value(cpu, mmu, name, imm);
    let res = val.rotate_left(4);
    set_operand_value(cpu, mmu, name, imm, res);
    cpu.set_flag_z(res == 0);
    cpu.set_flag_n(false);
    cpu.set_flag_h(false);
    cpu.set_flag_c(false);
}
