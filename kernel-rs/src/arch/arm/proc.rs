use crate::proc::UserProcInit;

/// A user program that calls exec("/init").
/// od -t xC initcode
pub const INITCODE: [u8; 80] = [
    0, 0x02, 0, 0x58, 0x21, 0x02, 0, 0x58, 0xe7, 0, 0x80, 0xd2, 0x01, 0, 0, 0xd4, 0x47, 0, 0x80,
    0xd2, 0x01, 0, 0, 0xd4, 0xfe, 0xff, 0xff, 0x17, 0x2f, 0x69, 0x6e, 0x69, 0x74, 0, 0, 0, 0x1f,
    0x20, 0x03, 0xd5, 0x1f, 0x20, 0x03, 0xd5, 0x1f, 0x20, 0x03, 0xd5, 0x1c, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0x1c, 0, 0, 0, 0, 0, 0, 0, 0x30, 0, 0, 0, 0, 0, 0, 0,
];

#[derive(Copy, Clone)]
pub struct TrapFrame {
    /// kernel page table (satp: Supervisor Address Translation and Protection)
    pub kernel_satp: usize,
    pub kernel_sp: usize,
    pub spsr: usize,
    pub fpsr: usize,

    /// 32 - usertrap()
    pub kernel_trap: usize,
    pub pc: usize, // user mode pc (elr_el1)
    pub sp: usize, // user mode sp

    /// 56
    pub r0: usize,
    pub r1: usize,
    pub r2: usize,
    pub r3: usize,
    pub r4: usize,
    pub r5: usize,
    pub r6: usize,
    pub r7: usize,
    pub r8: usize,
    pub r9: usize,
    pub r10: usize,
    pub r11: usize,
    pub r12: usize,
    pub r13: usize,
    pub r14: usize,
    pub r15: usize,
    pub r16: usize,
    pub r17: usize,
    pub r18: usize,
    pub r19: usize,
    pub r20: usize,
    pub r21: usize,
    pub r22: usize,
    pub r23: usize,
    pub r24: usize,
    pub r25: usize,
    pub r26: usize,
    pub r27: usize,
    pub r28: usize,
    pub r29: usize,
    pub r30: usize, // user mode lr

    // 304 - floating point registers
    pub s0: usize,
    pub s1: usize,
    pub s2: usize,
    pub s3: usize,
    pub s4: usize,
    pub s5: usize,
    pub s6: usize,
    pub s7: usize,
    pub s8: usize,
    pub s9: usize,
    pub s10: usize,
    pub s11: usize,
    pub s12: usize,
    pub s13: usize,
    pub s14: usize,
    pub s15: usize,
    pub s16: usize,
    pub s17: usize,
    pub s18: usize,
    pub s19: usize,
    pub s20: usize,
    pub s21: usize,
    pub s22: usize,
    pub s23: usize,
    pub s24: usize,
    pub s25: usize,
    pub s26: usize,
    pub s27: usize,
    pub s28: usize,
    pub s29: usize,
    pub s30: usize,
    pub s31: usize,
}
#[derive(Copy, Clone, Default)]
#[repr(C)]
pub struct Context {
    // svc mode registers
    pub r4: usize,
    pub r5: usize,
    pub r6: usize,
    pub r7: usize,
    pub r8: usize,
    pub r9: usize,
    pub r10: usize,
    pub r11: usize,
    pub r12: usize,
    pub r13: usize,
    pub r14: usize,
    pub r15: usize,
    pub r16: usize,
    pub r17: usize,
    pub r18: usize,
    pub r19: usize,
    pub r20: usize,
    pub r21: usize,
    pub r22: usize,
    pub r23: usize,
    pub r24: usize,
    pub r25: usize,
    pub r26: usize,
    pub r27: usize,
    pub r28: usize,
    pub r29: usize,
    pub lr: usize, // x30
    pub sp: usize,
}

impl TrapFrame {
    pub fn set_pc(&mut self, val: usize) {
        self.pc = val;
    }

    /// Set the value of return value register
    pub fn set_ret_val(&mut self, val: usize) {
        self.r0 = val;
    }

    /// Set the value of function argument register
    pub fn set_param_reg(&mut self, index: usize, val: usize) {
        let reg = match index {
            0 => &mut self.r0,
            1 => &mut self.r1,
            2 => &mut self.r2,
            3 => &mut self.r3,
            4 => &mut self.r4,
            5 => &mut self.r5,
            6 => &mut self.r6,
            7 => &mut self.r7,
            _ => panic!("Invalid Index!"),
        };
        *reg = val;
    }

    /// Get the value of function argument register
    pub fn get_param_reg(&self, index: usize) -> usize {
        match index {
            0 => self.r0,
            1 => self.r1,
            2 => self.r2,
            3 => self.r3,
            4 => self.r4,
            5 => self.r5,
            6 => self.r6,
            7 => self.r7,
            _ => panic!("Invalid Index!"),
        }
    }
}

impl Context {
    pub const fn new() -> Self {
        Self {
            r4: 0,
            r5: 0,
            r6: 0,
            r7: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            r16: 0,
            r17: 0,
            r18: 0,
            r19: 0,
            r20: 0,
            r21: 0,
            r22: 0,
            r23: 0,
            r24: 0,
            r25: 0,
            r26: 0,
            r27: 0,
            r28: 0,
            r29: 0,
            lr: 0, // x30
            sp: 0,
        }
    }

    /// Set return register (lr)
    pub fn set_ret_addr(&mut self, val: usize) {
        self.lr = val
    }
}

pub struct UserProcInitImpl;

impl UserProcInit for UserProcInitImpl {
    fn init_reg(trap_frame: &mut TrapFrame) {
        trap_frame.spsr = 0;
        trap_frame.fpsr = 0;
    }
}
