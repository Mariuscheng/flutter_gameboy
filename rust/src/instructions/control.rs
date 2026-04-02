//! 控制指令的處理模組
//!
//! 包含 NOP, STOP, HALT, DI, EI 等控制指令

use crate::cpu::Cpu;
use crate::mmu::Mmu;

/// 處理 NOP 指令
pub fn handle_nop(_cpu: &mut Cpu, _opcode: &crate::cpu::Opcode) {
    // 無操作
}

/// 處理 STOP 指令
pub fn handle_stop(cpu: &mut Cpu, mmu: &mut Mmu, _opcode: &crate::cpu::Opcode) {
    // STOP 指令 - 停止 CPU
    // 讀取 n8 操作數但不使用
    let _operand = cpu.fetch_byte(mmu);
    // STOP 不改變旗標
    // 在真實的 Game Boy 中，這會停止 CPU 直到發生中斷
    // 在模擬器中，我們可以簡單地繼續執行
}

/// 處理 DI 指令 (停用中斷)
pub fn handle_di(cpu: &mut Cpu, _mmu: &mut Mmu, _opcode: &crate::cpu::Opcode) {
    cpu.ime = crate::cpu::InterruptMasterState::Disabled;
}

/// 處理 EI 指令 (啟用中斷)
#[allow(dead_code)]
pub fn handle_ei(cpu: &mut Cpu, _mmu: &mut Mmu, _opcode: &crate::cpu::Opcode) {
    // EI 不會立即生效，而是在下一個指令之後啟用 IME
    cpu.ime = crate::cpu::InterruptMasterState::Pending;
}

/// 處理 SCF 指造 (設定進位旗標)
pub fn handle_scf(cpu: &mut Cpu, _mmu: &mut Mmu, _opcode: &crate::cpu::Opcode) {
    cpu.flags.c = crate::cpu::FlagState::Set;
    cpu.flags.n = crate::cpu::FlagState::Clear;
    cpu.flags.h = crate::cpu::FlagState::Clear;
}
