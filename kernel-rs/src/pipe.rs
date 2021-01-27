use crate::{
    file::{FileType, RcFile},
    kernel::kernel,
    page::Page,
    proc::{myproc, WaitChannel},
    riscv::PGSIZE,
    spinlock::Spinlock,
    vm::UVAddr,
};
use core::{mem, ops::Deref, ptr::NonNull};
use static_assertions::const_assert;

const PIPESIZE: usize = 512;

struct PipeInner {
    data: [u8; PIPESIZE],

    /// Number of bytes read.
    nread: u32,

    /// Number of bytes written.
    nwrite: u32,

    /// Read fd is still open.
    readopen: bool,

    /// Write fd is still open.
    writeopen: bool,
}

pub struct Pipe {
    inner: Spinlock<PipeInner>,

    /// WaitChannel for saying there are unread bytes in Pipe.data.
    read_waitchannel: WaitChannel,

    /// WaitChannel for saying all bytes in Pipe.data are already read.
    write_waitchannel: WaitChannel,
}

impl Pipe {
    /// PipeInner::try_read() tries to read as much as possible.
    /// Pipe::read() executes try_read() until all bytes in pipe are read.
    //TODO(https://github.com/kaist-cp/rv6/issues/366) : `n` should be u32.
    pub fn read(&self, addr: UVAddr, n: usize) -> Result<usize, ()> {
        let mut inner = self.inner.lock();
        loop {
            match unsafe { inner.try_read(addr, n) } {
                Ok(r) => {
                    //DOC: piperead-wakeup
                    self.write_waitchannel.wakeup();
                    return Ok(r);
                }
                Err(PipeError::WaitForIO) => {
                    //DOC: piperead-sleep
                    self.read_waitchannel.sleep(&mut inner);
                }
                _ => return Err(()),
            }
        }
    }

    /// PipeInner::try_write() tries to write as much as possible.
    /// Pipe::write() executes try_write() until `n` bytes are written.
    pub fn write(&self, addr: UVAddr, n: usize) -> Result<usize, ()> {
        let mut written = 0;
        let mut inner = self.inner.lock();
        loop {
            match unsafe { inner.try_write(addr + written, n - written) } {
                Ok(r) => {
                    written += r;
                    self.read_waitchannel.wakeup();
                    if written < n {
                        self.write_waitchannel.sleep(&mut inner);
                    } else {
                        return Ok(written);
                    }
                }
                Err(PipeError::InvalidCopyin(i)) => {
                    self.read_waitchannel.wakeup();
                    return Ok(written + i);
                }
                _ => return Err(()),
            }
        }
    }

    fn close(&self, writable: bool) -> bool {
        let mut inner = self.inner.lock();

        if writable {
            inner.writeopen = false;
            self.read_waitchannel.wakeup();
        } else {
            inner.readopen = false;
            self.write_waitchannel.wakeup();
        }

        // Return whether pipe should be freed or not.
        !inner.readopen && !inner.writeopen
    }
}

/// # Safety
///
/// `ptr` always refers to a `Pipe`.
pub struct AllocatedPipe {
    ptr: NonNull<Pipe>,
}

impl Deref for AllocatedPipe {
    type Target = Pipe;
    fn deref(&self) -> &Self::Target {
        // Safe since `ptr` always refers to a `Pipe`.
        unsafe { self.ptr.as_ref() }
    }
}

impl AllocatedPipe {
    pub fn alloc() -> Result<(RcFile<'static>, RcFile<'static>), ()> {
        let page = kernel().alloc().ok_or(())?;
        let mut ptr = NonNull::new(page.into_usize() as *mut Pipe).expect("AllocatedPipe alloc");

        // `Pipe` must be aligned with `Page`.
        const_assert!(mem::size_of::<Pipe>() <= PGSIZE);

        //TODO(https://github.com/kaist-cp/rv6/issues/367): Since Pipe is a huge struct, need to check whether stack is used to fill `*ptr`.
        unsafe {
            // Safe since `ptr` holds a valid, unique page allocated from `kernel().alloc()`,
            // and the pipe size and alignment are compatible with the page.
            *ptr.as_mut() = Pipe {
                inner: Spinlock::new(
                    "pipe",
                    PipeInner {
                        data: [0; PIPESIZE],
                        nwrite: 0,
                        nread: 0,
                        readopen: true,
                        writeopen: true,
                    },
                ),
                read_waitchannel: WaitChannel::new(),
                write_waitchannel: WaitChannel::new(),
            };
        }
        let f0 = kernel()
            .ftable
            .alloc_file(FileType::Pipe { pipe: Self { ptr } }, true, false)
            // Safe since ptr is an address of a page obtained by alloc().
            .map_err(|_| kernel().free(unsafe { Page::from_usize(ptr.as_ptr() as _) }))?;
        let f1 = kernel()
            .ftable
            .alloc_file(FileType::Pipe { pipe: Self { ptr } }, false, true)
            // Safe since ptr is an address of a page obtained by alloc().
            .map_err(|_| kernel().free(unsafe { Page::from_usize(ptr.as_ptr() as _) }))?;

        Ok((f0, f1))
    }

    pub fn close(self, writable: bool) {
        unsafe {
            // Safe since `ptr` holds a `Pipe` stored in a valid page allocated from `kernel().alloc()`.
            if self.ptr.as_ref().close(writable) {
                kernel().free(Page::from_usize(self.ptr.as_ptr() as _));
            }
        }
    }
}

pub enum PipeError {
    WaitForIO,
    InvalidStatus,
    InvalidCopyin(usize),
}

impl PipeInner {
    fn try_write(&mut self, addr: UVAddr, n: usize) -> Result<usize, PipeError> {
        let mut ch = [0u8];
        let proc = unsafe {
            // TODO(https://github.com/kaist-cp/rv6/issues/354)
            // Remove this unsafe part after resolving #354.
            &*myproc()
        };
        if !self.readopen || proc.killed() {
            return Err(PipeError::InvalidStatus);
        }

        let data = unsafe {
            // TODO(https://github.com/kaist-cp/rv6/issues/354)
            // Remove this unsafe part after resolving #354.
            &mut *proc.data.get()
        };
        for i in 0..n {
            if self.nwrite == self.nread.wrapping_add(PIPESIZE as u32) {
                //DOC: pipewrite-full
                return Ok(i);
            }
            if data.memory.copy_in(&mut ch, addr + i).is_err() {
                return Err(PipeError::InvalidCopyin(i));
            }
            self.data[self.nwrite as usize % PIPESIZE] = ch[0];
            self.nwrite = self.nwrite.wrapping_add(1);
        }
        Ok(n)
    }

    fn try_read(&mut self, addr: UVAddr, n: usize) -> Result<usize, PipeError> {
        let proc = unsafe {
            // TODO(https://github.com/kaist-cp/rv6/issues/354)
            // Remove this unsafe part after resolving #354.
            &*myproc()
        };
        //DOC: pipe-empty
        if self.nread == self.nwrite && self.writeopen {
            if proc.killed() {
                return Err(PipeError::InvalidStatus);
            }
            return Err(PipeError::WaitForIO);
        }

        let data = unsafe {
            // TODO(https://github.com/kaist-cp/rv6/issues/354)
            // Remove this unsafe part after resolving #354.
            &mut *proc.data.get()
        };
        //DOC: piperead-copy
        for i in 0..n {
            if self.nread == self.nwrite {
                return Ok(i);
            }
            let ch = [self.data[self.nread as usize % PIPESIZE]];
            self.nread = self.nread.wrapping_add(1);
            if data.memory.copy_out(addr + i, &ch).is_err() {
                return Ok(i);
            }
        }
        Ok(n)
    }
}
