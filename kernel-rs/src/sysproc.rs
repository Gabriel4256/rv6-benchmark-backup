use core::{mem, slice};

use crate::{
    kernel::Kernel,
    poweroff,
    proc::{myproc, resizeproc},
    syscall::{argaddr, argint},
    vm::{UVAddr, VAddr},
};

impl Kernel {
    pub unsafe fn sys_exit(&self) -> Result<usize, ()> {
        let n = argint(0)?;
        self.procs.exit_current(n);
    }

    pub unsafe fn sys_getpid(&self) -> Result<usize, ()> {
        Ok((*myproc()).pid() as _)
    }

    pub unsafe fn sys_fork(&self) -> Result<usize, ()> {
        Ok(self.procs.fork()? as _)
    }

    pub unsafe fn sys_wait(&self) -> Result<usize, ()> {
        let p = argaddr(0)?;
        Ok(self.procs.wait(UVAddr::new(p))? as _)
    }

    pub unsafe fn sys_sbrk(&self) -> Result<usize, ()> {
        let n = argint(0)?;
        let addr = (*(*myproc()).data.get()).sz as i32;
        if resizeproc(n) < 0 {
            return Err(());
        }
        Ok(addr as usize)
    }

    pub unsafe fn sys_sleep(&self) -> Result<usize, ()> {
        let n = argint(0)?;
        let mut ticks = self.ticks.lock();
        let ticks0 = *ticks;
        while ticks.wrapping_sub(ticks0) < n as u32 {
            if (*myproc()).killed() {
                return Err(());
            }
            ticks.sleep();
        }
        Ok(0)
    }

    pub unsafe fn sys_kill(&self) -> Result<usize, ()> {
        let pid = argint(0)?;
        Ok(self.procs.kill(pid)? as usize)
    }

    /// Return how many clock tick interrupts have occurred
    /// since start.
    pub unsafe fn sys_uptime(&self) -> Result<usize, ()> {
        Ok(*self.ticks.lock() as usize)
    }

    pub unsafe fn sys_poweroff(&self) -> Result<usize, ()> {
        let exitcode = argint(0)?;
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
