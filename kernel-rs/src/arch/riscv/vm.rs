use bitflags::bitflags;

use crate::{
    addr::{PAddr, PGSIZE},
    arch::{
        addr::{pa2pte, pte2pa},
        asm::{make_satp, sfence_vma, w_satp},
        memlayout::{FINISHER, PLIC},
    },
    vm::{AccessFlags, PageInitiator, PageTableEntryDesc, RawPageTable},
};

bitflags! {
    pub struct RiscVPteFlags: usize {
        /// valid
        const V = 1 << 0;
        /// readable
    const R = 1 << 1;
        /// writable
        const W = 1 << 2;
        /// executable
        const X = 1 << 3;
        /// user-accessible
        const U = 1 << 4;
    }
}

pub type PteFlags = RiscVPteFlags;

impl From<AccessFlags> for RiscVPteFlags {
    fn from(item: AccessFlags) -> Self {
        let mut ret = Self::empty();
        if item.intersects(AccessFlags::R) {
            ret |= Self::R;
        }
        if item.intersects(AccessFlags::W) {
            ret |= Self::W;
        }
        if item.intersects(AccessFlags::X) {
            ret |= Self::X;
        }
        if item.intersects(AccessFlags::U) {
            ret |= Self::U;
        }
        ret
    }
}

/// # Safety
///
/// If self.is_table() is true, then it must refer to a valid page-table page.
///
/// Because of #[derive(Default)], inner is initially 0, which satisfies the invariant.
#[derive(Default)]
pub struct RiscVPageTableEntry {
    inner: usize,
}

pub type PageTableEntry = RiscVPageTableEntry;

impl PageTableEntryDesc for RiscVPageTableEntry {
    type EntryFlags = RiscVPteFlags;

    fn get_flags(&self) -> Self::EntryFlags {
        Self::EntryFlags::from_bits_truncate(self.inner)
    }

    fn flag_intersects(&self, flag: Self::EntryFlags) -> bool {
        self.get_flags().intersects(flag)
    }

    fn get_pa(&self) -> PAddr {
        pte2pa(self.inner)
    }

    fn is_valid(&self) -> bool {
        self.flag_intersects(Self::EntryFlags::V)
    }

    fn is_user(&self) -> bool {
        self.flag_intersects(Self::EntryFlags::V | Self::EntryFlags::U)
    }

    fn is_table(&self) -> bool {
        self.is_valid()
            && !self
                .flag_intersects(Self::EntryFlags::R | Self::EntryFlags::W | Self::EntryFlags::X)
    }

    fn is_data(&self) -> bool {
        self.is_valid()
            && self.flag_intersects(Self::EntryFlags::R | Self::EntryFlags::W | Self::EntryFlags::X)
    }

    /// Make the entry refer to a given page-table page.
    fn set_table(&mut self, page: *mut RawPageTable) {
        self.inner = pa2pte((page as usize).into()) | Self::EntryFlags::V.bits();
    }

    /// Make the entry refer to a given address with a given permission.
    /// The permission should include at lease one of R, W, and X not to be
    /// considered as an entry referring a page-table page.
    fn set_entry(&mut self, pa: PAddr, perm: Self::EntryFlags) {
        assert!(perm.intersects(Self::EntryFlags::R | Self::EntryFlags::W | Self::EntryFlags::X));
        self.inner = pa2pte(pa) | (perm | Self::EntryFlags::V).bits();
    }

    /// Make the entry inaccessible by user processes by clearing RiscVPteFlags::U.
    fn clear_user(&mut self) {
        self.inner &= !(Self::EntryFlags::U.bits());
    }

    /// Invalidate the entry by making every bit 0.
    fn invalidate(&mut self) {
        self.inner = 0;
    }
}

pub struct RiscVPageInit;

impl RiscVPageInit {
    // Device mappings in memory.
    // SiFive Test Finisher MMIO, PLIC.
    const DEV_MAPPING: [(usize, usize); 2] = [(FINISHER, PGSIZE), (PLIC, 0x400000)];
}

pub type PageInit = RiscVPageInit;

impl PageInitiator for RiscVPageInit {

    fn kernel_page_dev_mappings() ->&'static [(usize, usize)]{
        &Self::DEV_MAPPING[0..2]
    }

    unsafe fn switch_page_table_and_enable_mmu(page_table_base: usize) {
        unsafe {
            w_satp(make_satp(page_table_base));
            sfence_vma();
        }
    }
}
