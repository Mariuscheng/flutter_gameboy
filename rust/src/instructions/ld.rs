//! 載入指令 (LD) 的處理模組
//!
//! 這個模組負責處理所有 LD 指令的變體

use crate::cpu::Cpu;
use crate::mmu::Mmu;

/// 處理載入指令
pub fn handle_ld(cpu: &mut Cpu, mmu: &mut Mmu, opcode: &crate::cpu::Opcode) {
    // 特殊處理：LD HL, SP+e8 (opcode 0xF8) 有 3 個 operands
    if opcode.operands.len() == 3 {
        let op0 = &opcode.operands[0];
        let op1 = &opcode.operands[1];
        let op2 = &opcode.operands[2];

        // LD HL, SP+e8: operands[0]=HL, operands[1]=SP (increment=true), operands[2]=e8
        if op0.name == "HL" && op1.name == "SP" && op2.name == "e8" {
            let raw_offset = cpu.fetch_byte(mmu);
            let offset = raw_offset as i8 as i16 as u16;
            let sp = cpu.sp;
            let res = sp.wrapping_add(offset);

            cpu.flags.z = crate::cpu::FlagState::Clear;
            cpu.flags.n = crate::cpu::FlagState::Clear;
            // H and C flags are calculated based on the unsigned low byte of SP and unsigned offset
            cpu.flags.h = if (sp & 0xF) + (raw_offset as u16 & 0xF) > 0xF {
                crate::cpu::FlagState::Set
            } else {
                crate::cpu::FlagState::Clear
            };
            cpu.flags.c = if (sp & 0xFF) + (raw_offset as u16) > 0xFF {
                crate::cpu::FlagState::Set
            } else {
                crate::cpu::FlagState::Clear
            };

            cpu.set_hl(res);
            return;
        }
    }

    if opcode.operands.len() >= 2 {
        let op0 = &opcode.operands[0];
        let op1 = &opcode.operands[1];

        match (
            op0.name.as_str(),
            op0.immediate,
            op0.increment,
            op0.decrement,
            op1.name.as_str(),
            op1.immediate,
            op1.increment,
            op1.decrement,
        ) {
            // 載入立即數到暫存器
            ("B", Some(true), _, _, "n8", _, _, _) => {
                let value = cpu.fetch_byte(mmu);
                cpu.registers.b = value;
            }
            ("C", Some(true), _, _, "n8", _, _, _) => {
                let value = cpu.fetch_byte(mmu);
                cpu.registers.c = value;
            }
            ("D", Some(true), _, _, "n8", _, _, _) => {
                let value = cpu.fetch_byte(mmu);
                cpu.registers.d = value;
            }
            ("E", Some(true), _, _, "n8", _, _, _) => {
                let value = cpu.fetch_byte(mmu);
                cpu.registers.e = value;
            }
            ("H", Some(true), _, _, "n8", _, _, _) => {
                let value = cpu.fetch_byte(mmu);
                cpu.registers.h = value;
            }
            ("L", Some(true), _, _, "n8", _, _, _) => {
                let value = cpu.fetch_byte(mmu);
                cpu.registers.l = value;
            }
            ("A", Some(true), _, _, "n8", _, _, _) => {
                let value = cpu.fetch_byte(mmu);
                cpu.registers.a = value;
            }

            // 載入 16 位元立即數
            ("SP", Some(true), _, _, "n16", _, _, _) => {
                let value = cpu.fetch_word(mmu);
                cpu.sp = value;
            }
            ("BC", Some(true), _, _, "n16", _, _, _) => {
                let value = cpu.fetch_word(mmu);
                cpu.registers.b = (value >> 8) as u8;
                cpu.registers.c = value as u8;
            }
            ("DE", Some(true), _, _, "n16", _, _, _) => {
                let value = cpu.fetch_word(mmu);
                cpu.registers.d = (value >> 8) as u8;
                cpu.registers.e = value as u8;
            }
            ("HL", Some(true), _, _, "n16", _, _, _) => {
                let value = cpu.fetch_word(mmu);
                cpu.registers.h = (value >> 8) as u8;
                cpu.registers.l = value as u8;
            }

            // 從記憶體載入到暫存器
            ("A", Some(true), _, _, "a16", _, _, _) => {
                let addr = cpu.fetch_word(mmu);
                cpu.registers.a = mmu.read_byte(addr);
            }
            ("A", Some(true), _, _, "BC", Some(false), _, _) => {
                let addr = ((cpu.registers.b as u16) << 8) | (cpu.registers.c as u16);
                cpu.registers.a = mmu.read_byte(addr);
            }
            ("A", Some(true), _, _, "DE", Some(false), _, _)
            | ("A", Some(true), _, _, "DE", Some(true), _, _) => {
                // 有些 ROM 表格中 DE 可能被標記為 immediate true 但它其實是 address
                let addr = ((cpu.registers.d as u16) << 8) | (cpu.registers.e as u16);
                cpu.registers.a = mmu.read_byte(addr);
            }
            // 普通 LD A, [HL] (沒有自增/自減)
            ("A", Some(true), _, _, "HL", Some(false), None, None) => {
                let addr = ((cpu.registers.h as u16) << 8) | (cpu.registers.l as u16);
                cpu.registers.a = mmu.read_byte(addr);
            }
            ("B", Some(true), _, _, "HL", Some(false), None, None) => {
                let addr = ((cpu.registers.h as u16) << 8) | (cpu.registers.l as u16);
                cpu.registers.b = mmu.read_byte(addr);
            }
            ("C", Some(true), _, _, "HL", Some(false), None, None) => {
                let addr = ((cpu.registers.h as u16) << 8) | (cpu.registers.l as u16);
                cpu.registers.c = mmu.read_byte(addr);
            }
            ("D", Some(true), _, _, "HL", Some(false), None, None) => {
                let addr = ((cpu.registers.h as u16) << 8) | (cpu.registers.l as u16);
                cpu.registers.d = mmu.read_byte(addr);
            }
            ("E", Some(true), _, _, "HL", Some(false), None, None) => {
                let addr = ((cpu.registers.h as u16) << 8) | (cpu.registers.l as u16);
                cpu.registers.e = mmu.read_byte(addr);
            }
            ("H", Some(true), _, _, "HL", Some(false), None, None) => {
                let addr = ((cpu.registers.h as u16) << 8) | (cpu.registers.l as u16);
                cpu.registers.h = mmu.read_byte(addr);
            }
            ("L", Some(true), _, _, "HL", Some(false), None, None) => {
                let addr = ((cpu.registers.h as u16) << 8) | (cpu.registers.l as u16);
                cpu.registers.l = mmu.read_byte(addr);
            }

            // 載入到記憶體
            ("a16", _, _, _, "A", Some(true), _, _) => {
                let addr = cpu.fetch_word(mmu);
                mmu.write_byte(addr, cpu.registers.a);
            }
            ("HL", Some(false), None, None, "A", Some(true), _, _) => {
                let addr = ((cpu.registers.h as u16) << 8) | (cpu.registers.l as u16);
                mmu.write_byte(addr, cpu.registers.a);
            }
            ("HL", Some(false), None, None, "n8", _, _, _) => {
                let value = cpu.fetch_byte(mmu);
                let addr = ((cpu.registers.h as u16) << 8) | (cpu.registers.l as u16);
                mmu.write_byte(addr, value);
            }
            ("BC", Some(false), _, _, "A", Some(true), _, _) => {
                let addr = ((cpu.registers.b as u16) << 8) | (cpu.registers.c as u16);
                mmu.write_byte(addr, cpu.registers.a);
            }
            ("DE", Some(false), _, _, "A", Some(true), _, _) => {
                let addr = ((cpu.registers.d as u16) << 8) | (cpu.registers.e as u16);
                mmu.write_byte(addr, cpu.registers.a);
            }
            ("HL", Some(false), None, None, "B", Some(true), _, _) => {
                let addr = ((cpu.registers.h as u16) << 8) | (cpu.registers.l as u16);
                mmu.write_byte(addr, cpu.registers.b);
            }
            ("HL", Some(false), None, None, "C", Some(true), _, _) => {
                let addr = ((cpu.registers.h as u16) << 8) | (cpu.registers.l as u16);
                mmu.write_byte(addr, cpu.registers.c);
            }
            ("HL", Some(false), None, None, "D", Some(true), _, _) => {
                let addr = ((cpu.registers.h as u16) << 8) | (cpu.registers.l as u16);
                mmu.write_byte(addr, cpu.registers.d);
            }
            ("HL", Some(false), None, None, "E", Some(true), _, _) => {
                let addr = ((cpu.registers.h as u16) << 8) | (cpu.registers.l as u16);
                mmu.write_byte(addr, cpu.registers.e);
            }
            ("HL", Some(false), None, None, "H", Some(true), _, _) => {
                let addr = ((cpu.registers.h as u16) << 8) | (cpu.registers.l as u16);
                mmu.write_byte(addr, cpu.registers.h);
            }
            ("HL", Some(false), None, None, "L", Some(true), _, _) => {
                let addr = ((cpu.registers.h as u16) << 8) | (cpu.registers.l as u16);
                mmu.write_byte(addr, cpu.registers.l);
            }

            // 暫存器間傳輸 - 8 位元暫存器
            ("A", Some(true), _, _, "A", Some(true), _, _) => {
                // A = A, no operation needed
            }
            ("A", Some(true), _, _, "B", Some(true), _, _) => {
                cpu.registers.a = cpu.registers.b;
            }
            ("A", Some(true), _, _, "C", Some(true), _, _) => {
                cpu.registers.a = cpu.registers.c;
            }
            ("A", Some(true), _, _, "D", Some(true), _, _) => {
                cpu.registers.a = cpu.registers.d;
            }
            ("A", Some(true), _, _, "E", Some(true), _, _) => {
                cpu.registers.a = cpu.registers.e;
            }
            ("A", Some(true), _, _, "H", Some(true), _, _) => {
                cpu.registers.a = cpu.registers.h;
            }
            ("A", Some(true), _, _, "L", Some(true), _, _) => {
                cpu.registers.a = cpu.registers.l;
            }

            ("B", Some(true), _, _, "A", Some(true), _, _) => {
                cpu.registers.b = cpu.registers.a;
            }
            ("B", Some(true), _, _, "B", Some(true), _, _) => {
                // B = B, no operation needed
            }
            ("B", Some(true), _, _, "C", Some(true), _, _) => {
                cpu.registers.b = cpu.registers.c;
            }
            ("B", Some(true), _, _, "D", Some(true), _, _) => {
                cpu.registers.b = cpu.registers.d;
            }
            ("B", Some(true), _, _, "E", Some(true), _, _) => {
                cpu.registers.b = cpu.registers.e;
            }
            ("B", Some(true), _, _, "H", Some(true), _, _) => {
                cpu.registers.b = cpu.registers.h;
            }
            ("B", Some(true), _, _, "L", Some(true), _, _) => {
                cpu.registers.b = cpu.registers.l;
            }

            ("C", Some(true), _, _, "A", Some(true), _, _) => {
                cpu.registers.c = cpu.registers.a;
            }
            ("C", Some(true), _, _, "B", Some(true), _, _) => {
                cpu.registers.c = cpu.registers.b;
            }
            ("C", Some(true), _, _, "C", Some(true), _, _) => {
                // C = C, no operation needed
            }
            ("C", Some(true), _, _, "D", Some(true), _, _) => {
                cpu.registers.c = cpu.registers.d;
            }
            ("C", Some(true), _, _, "E", Some(true), _, _) => {
                cpu.registers.c = cpu.registers.e;
            }
            ("C", Some(true), _, _, "H", Some(true), _, _) => {
                cpu.registers.c = cpu.registers.h;
            }
            ("C", Some(true), _, _, "L", Some(true), _, _) => {
                cpu.registers.c = cpu.registers.l;
            }

            ("D", Some(true), _, _, "A", Some(true), _, _) => {
                cpu.registers.d = cpu.registers.a;
            }
            ("D", Some(true), _, _, "B", Some(true), _, _) => {
                cpu.registers.d = cpu.registers.b;
            }
            ("D", Some(true), _, _, "C", Some(true), _, _) => {
                cpu.registers.d = cpu.registers.c;
            }
            ("D", Some(true), _, _, "D", Some(true), _, _) => {
                // D = D, no operation needed
            }
            ("D", Some(true), _, _, "E", Some(true), _, _) => {
                cpu.registers.d = cpu.registers.e;
            }
            ("D", Some(true), _, _, "H", Some(true), _, _) => {
                cpu.registers.d = cpu.registers.h;
            }
            ("D", Some(true), _, _, "L", Some(true), _, _) => {
                cpu.registers.d = cpu.registers.l;
            }

            ("E", Some(true), _, _, "A", Some(true), _, _) => {
                cpu.registers.e = cpu.registers.a;
            }
            ("E", Some(true), _, _, "B", Some(true), _, _) => {
                cpu.registers.e = cpu.registers.b;
            }
            ("E", Some(true), _, _, "C", Some(true), _, _) => {
                cpu.registers.e = cpu.registers.c;
            }
            ("E", Some(true), _, _, "D", Some(true), _, _) => {
                cpu.registers.e = cpu.registers.d;
            }
            ("E", Some(true), _, _, "E", Some(true), _, _) => {
                // E = E, no operation needed
            }
            ("E", Some(true), _, _, "H", Some(true), _, _) => {
                cpu.registers.e = cpu.registers.h;
            }
            ("E", Some(true), _, _, "L", Some(true), _, _) => {
                cpu.registers.e = cpu.registers.l;
            }

            ("H", Some(true), _, _, "A", Some(true), _, _) => {
                cpu.registers.h = cpu.registers.a;
            }
            ("H", Some(true), _, _, "B", Some(true), _, _) => {
                cpu.registers.h = cpu.registers.b;
            }
            ("H", Some(true), _, _, "C", Some(true), _, _) => {
                cpu.registers.h = cpu.registers.c;
            }
            ("H", Some(true), _, _, "D", Some(true), _, _) => {
                cpu.registers.h = cpu.registers.d;
            }
            ("H", Some(true), _, _, "E", Some(true), _, _) => {
                cpu.registers.h = cpu.registers.e;
            }
            ("H", Some(true), _, _, "H", Some(true), _, _) => {
                // H = H, no operation needed
            }
            ("H", Some(true), _, _, "L", Some(true), _, _) => {
                cpu.registers.h = cpu.registers.l;
            }

            ("L", Some(true), _, _, "A", Some(true), _, _) => {
                cpu.registers.l = cpu.registers.a;
            }
            ("L", Some(true), _, _, "B", Some(true), _, _) => {
                cpu.registers.l = cpu.registers.b;
            }
            ("L", Some(true), _, _, "C", Some(true), _, _) => {
                cpu.registers.l = cpu.registers.c;
            }
            ("L", Some(true), _, _, "D", Some(true), _, _) => {
                cpu.registers.l = cpu.registers.d;
            }
            ("L", Some(true), _, _, "E", Some(true), _, _) => {
                cpu.registers.l = cpu.registers.e;
            }
            ("L", Some(true), _, _, "H", Some(true), _, _) => {
                cpu.registers.l = cpu.registers.h;
            }
            ("L", Some(true), _, _, "L", Some(true), _, _) => {
                // L = L, no operation needed
            }

            // 特殊載入指令 (自增/自減)
            ("HL", _, Some(true), _, "A", Some(true), _, _) => {
                let addr = cpu.get_hl();
                mmu.write_byte(addr, cpu.registers.a);
                cpu.set_hl(addr.wrapping_add(1));
            }
            ("A", Some(true), _, _, "HL", _, Some(true), _) => {
                let addr = cpu.get_hl();
                cpu.registers.a = mmu.read_byte(addr);
                cpu.set_hl(addr.wrapping_add(1));
            }
            ("HL", _, _, Some(true), "A", Some(true), _, _) => {
                let addr = cpu.get_hl();
                mmu.write_byte(addr, cpu.registers.a);
                cpu.set_hl(addr.wrapping_sub(1));
            }
            ("A", Some(true), _, _, "HL", _, _, Some(true)) => {
                let addr = cpu.get_hl();
                cpu.registers.a = mmu.read_byte(addr);
                cpu.set_hl(addr.wrapping_sub(1));
            }

            // 下面是相容舊版結構的操作
            ("HLI", Some(false), _, _, "A", Some(true), _, _)
            | ("HL+", Some(false), _, _, "A", Some(true), _, _) => {
                let addr = cpu.get_hl();
                mmu.write_byte(addr, cpu.registers.a);
                cpu.set_hl(addr.wrapping_add(1));
            }
            ("A", Some(true), _, _, "HLI", Some(false), _, _)
            | ("A", Some(true), _, _, "HL+", Some(false), _, _) => {
                let addr = cpu.get_hl();
                cpu.registers.a = mmu.read_byte(addr);
                cpu.set_hl(addr.wrapping_add(1));
            }
            ("HLD", Some(false), _, _, "A", Some(true), _, _)
            | ("HL-", Some(false), _, _, "A", Some(true), _, _) => {
                let addr = cpu.get_hl();
                mmu.write_byte(addr, cpu.registers.a);
                cpu.set_hl(addr.wrapping_sub(1));
            }
            ("A", Some(true), _, _, "HLD", Some(false), _, _)
            | ("A", Some(true), _, _, "HL-", Some(false), _, _) => {
                let addr = cpu.get_hl();
                cpu.registers.a = mmu.read_byte(addr);
                cpu.set_hl(addr.wrapping_sub(1));
            }

            ("a16", _, _, _, "SP", Some(true), _, _) => {
                let addr = cpu.fetch_word(mmu);
                let sp_low = cpu.sp as u8;
                let sp_high = (cpu.sp >> 8) as u8;
                mmu.write_byte(addr, sp_low);
                mmu.write_byte(addr.wrapping_add(1), sp_high);
            }
            ("AF", Some(true), _, _, "n16", _, _, _) => {
                let value = cpu.fetch_word(mmu);
                cpu.set_af(value);
            }
            ("SP", Some(true), _, _, "HL", Some(true), _, _) => {
                cpu.sp = cpu.get_hl();
            }

            _ => {
                // LD 變體未實作时保持沉默
            }
        }
    }
}
