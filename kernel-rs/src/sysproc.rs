use core::{mem, slice};

use crate::{
    kernel::Kernel,
    ok_or, poweroff,
    proc::{myproc, resizeproc},
    syscall::{argaddr, argint},
    vm::{UVAddr, VAddr},
};

impl Kernel {
    pub unsafe fn sys_exit(&self) -> usize {
        let n = ok_or!(argint(0), return usize::MAX);
        self.procs.exit_current(n);
    }

    pub unsafe fn sys_getpid(&self) -> usize {
        (*myproc()).pid() as _
    }

    pub unsafe fn sys_fork(&self) -> usize {
        self.procs.fork() as _
    }

    pub unsafe fn sys_wait(&self) -> usize {
        let p = ok_or!(argaddr(0), return usize::MAX);
        self.procs.wait(UVAddr::new(p)) as _
    }

    pub unsafe fn sys_sbrk(&self) -> usize {
        let n = ok_or!(argint(0), return usize::MAX);
        let addr: i32 = (*(*myproc()).data.get()).sz as i32;
        if resizeproc(n) < 0 {
            return usize::MAX;
        }
        addr as usize
    }

    pub unsafe fn sys_sleep(&self) -> usize {
        let n = ok_or!(argint(0), return usize::MAX);
        let mut ticks = self.ticks.lock();
        let ticks0 = *ticks;
        while ticks.wrapping_sub(ticks0) < n as u32 {
            if (*myproc()).killed() {
                return usize::MAX;
            }
            ticks.sleep();
        }
        0
    }

    pub unsafe fn sys_kill(&self) -> usize {
        let pid = ok_or!(argint(0), return usize::MAX);
        self.procs.kill(pid) as usize
    }

    /// return how many clock tick interrupts have occurred
    /// since start.
    pub unsafe fn sys_uptime(&self) -> usize {
        *self.ticks.lock() as usize
    }

    pub unsafe fn sys_poweroff(&self) -> usize {
        let exitcode = ok_or!(argint(0), return usize::MAX);
        poweroff::machine_poweroff(exitcode as _);
    }

    pub fn sys_clock(&self) -> usize {
        let p = unsafe { argaddr(0).unwrap() };
        let addr = UVAddr::new(p);

        let mut x;
        unsafe {
            asm!("rdcycle {}", out(reg) x);
        };

        let mut clk = x;
        let data = unsafe { &mut *(*myproc()).data.get() } ;
        let tmp = unsafe {
            slice::from_raw_parts_mut(
                &mut clk as *mut i32 as *mut u8,
                mem::size_of::<i32>())
        };
        unsafe {
            data.pagetable.copy_out(addr, tmp).unwrap();
        }

        0
    }
}
