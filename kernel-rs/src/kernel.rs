use core::sync::atomic::{spin_loop_hint, AtomicBool, Ordering};
use core::{
    fmt::{self, Write},
    mem, ptr,
};

use crate::{
    bio::binit,
    console::{consoleinit, Console},
    kalloc::{end, kinit, Kmem},
    memlayout::PHYSTOP,
    param::NCPU,
    plic::{plicinit, plicinithart},
    println,
    proc::{cpuid, procinit, scheduler, Cpu, ProcessSystem},
    riscv::PGSIZE,
    sleepablelock::Sleepablelock,
    spinlock::Spinlock,
    trap::{trapinit, trapinithart},
    uart::Uart,
    virtio_disk::virtio_disk_init,
    vm::{kvminit, kvminithart, PageTable},
};

/// The kernel.
static mut KERNEL: mem::MaybeUninit<Kernel> = mem::MaybeUninit::uninit();

/// The kernel can be mutably accessed only during the initialization.
unsafe fn kernel_mut() -> &'static mut Kernel {
    &mut *KERNEL.as_mut_ptr()
}

/// After intialized, the kernel is safe to immutably access.
pub fn kernel() -> &'static Kernel {
    unsafe { &*KERNEL.as_ptr() }
}

pub struct Kernel {
    panicked: AtomicBool,

    /// Sleeps waiting for there are some input in console buffer.
    pub console: Sleepablelock<Console>,

    kmem: Spinlock<Kmem>,

    /// The kernel's page table.
    pub page_table: PageTable,

    pub ticks: Sleepablelock<u32>,

    /// Current process system.
    pub procs: ProcessSystem,

    pub cpus: [Cpu; NCPU],
}

impl Kernel {
    fn panic(&self) {
        self.panicked.store(true, Ordering::Release);
    }

    pub fn is_panicked(&self) -> bool {
        self.panicked.load(Ordering::Acquire)
    }

    /// Free the page of physical memory pointed at by v,
    /// which normally should have been returned by a
    /// call to kernel().alloc().  (The exception is when
    /// initializing the allocator; see kinit above.)
    pub unsafe fn free(&self, pa: *mut u8) {
        if (pa as usize).wrapping_rem(PGSIZE) != 0
            || pa < end.as_mut_ptr()
            || pa as usize >= PHYSTOP
        {
            panic!("Kernel::free");
        }

        // Fill with junk to catch dangling refs.
        ptr::write_bytes(pa, 1, PGSIZE);

        kernel().kmem.lock().free(pa);
    }

    /// Allocate one 4096-byte page of physical memory.
    /// Returns a pointer that the kernel can use.
    /// Returns 0 if the memory cannot be allocated.
    pub unsafe fn alloc(&self) -> *mut u8 {
        let ret = kernel().kmem.lock().alloc();
        if ret.is_null() {
            return ret;
        }

        // fill with junk
        ptr::write_bytes(ret, 5, PGSIZE);
        ret
    }

    pub fn console_write_fmt(&self, args: fmt::Arguments<'_>) -> fmt::Result {
        if self.is_panicked() {
            unsafe { kernel().console.get_mut_unchecked().write_fmt(args) }
        } else {
            let mut lock = kernel().console.lock();
            lock.write_fmt(args)
        }
    }
}

/// Handles panic.
#[cfg(not(test))]
#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo<'_>) -> ! {
    // Freeze other CPUs.
    kernel().panic();
    println!("{}", info);

    crate::utils::spin_loop()
}

/// start() jumps here in supervisor mode on all CPUs.
pub unsafe fn kernel_main() -> ! {
    static STARTED: AtomicBool = AtomicBool::new(false);

    if cpuid() == 0 {
        // Initialize the kernel.

        // Console.
        consoleinit(&mut kernel_mut().console, Uart::new());

        println!();
        println!("rv6 kernel is booting");
        println!();

        // Physical page allocator.
        kinit(&mut kernel_mut().kmem);

        // Create kernel page table.
        kvminit(&mut kernel_mut().page_table);

        // Turn on paging.
        kvminithart(&kernel().page_table);

        // Process system.
        procinit(
            &mut kernel_mut().procs,
            &mut kernel_mut().cpus,
            &mut kernel_mut().page_table,
        );

        // Trap vectors.
        trapinit(&mut kernel_mut().ticks);

        // Install kernel trap vector.
        trapinithart();

        // Set up interrupt controller.
        plicinit();

        // Ask PLIC for device interrupts.
        plicinithart();

        // Buffer cache.
        binit();

        // Emulated hard disk.
        virtio_disk_init();

        // First user process.
        kernel_mut().procs.user_proc_init();
        STARTED.store(true, Ordering::Release);
    } else {
        while !STARTED.load(Ordering::Acquire) {
            spin_loop_hint();
        }

        println!("hart {} starting", cpuid());

        // Turn on paging.
        kvminithart(&kernel().page_table);

        // Install kernel trap vector.
        trapinithart();

        // Ask PLIC for device interrupts.
        plicinithart();
    }

    scheduler()
}