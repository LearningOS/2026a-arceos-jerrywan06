#![no_std]

use allocator::{AllocError, AllocResult, BaseAllocator, ByteAllocator, PageAllocator};
use core::alloc::Layout;
use core::ptr::NonNull;

/// Early memory allocator
/// Use it before formal bytes-allocator and pages-allocator can work!
/// This is a double-end memory range:
/// - Alloc bytes forward
/// - Alloc pages backward
///
/// [ bytes-used | avail-area | pages-used ]
/// |            | -->    <-- |            |
/// start    byte_cursor  page_cursor   end
///
/// For bytes area, 'count' records number of allocations.
/// When it goes down to ZERO, free bytes-used area.
/// For pages area, it will never be freed!
///
pub struct EarlyAllocator<const SIZE: usize> {
    start: usize,
    end: usize,
    byte_cursor: usize,
    page_cursor: usize,
    byte_allocations: usize,
}

impl<const SIZE: usize> EarlyAllocator<SIZE> {
    pub const fn new() -> Self {
        assert!(SIZE.is_power_of_two());
        Self {
            start: 0,
            end: 0,
            byte_cursor: 0,
            page_cursor: 0,
            byte_allocations: 0,
        }
    }
}

impl<const SIZE: usize> BaseAllocator for EarlyAllocator<SIZE> {
    fn init(&mut self, start: usize, size: usize) {
        let end = start.checked_add(size).expect("allocator range overflow");
        self.start = start;
        self.end = end;
        self.byte_cursor = start;
        self.page_cursor = end;
        self.byte_allocations = 0;
    }

    fn add_memory(&mut self, start: usize, size: usize) -> AllocResult {
        let end = start.checked_add(size).ok_or(AllocError::InvalidParam)?;
        if size == 0 {
            return Err(AllocError::InvalidParam);
        }
        if start < self.end && self.start < end {
            return Err(AllocError::MemoryOverlap);
        }
        // Only an unused end of this contiguous range can be extended.
        if start == self.end && self.page_cursor == self.end {
            self.end = end;
            self.page_cursor = end;
        } else if end == self.start && self.byte_allocations == 0 {
            self.start = start;
            self.byte_cursor = start;
        } else {
            return Err(AllocError::InvalidParam);
        }
        Ok(())
    }
}

impl<const SIZE: usize> ByteAllocator for EarlyAllocator<SIZE> {
    fn alloc(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        let align_mask = layout.align() - 1;
        let start = self
            .byte_cursor
            .checked_add(align_mask)
            .ok_or(AllocError::NoMemory)?
            & !align_mask;
        let end = start
            .checked_add(layout.size().max(1))
            .ok_or(AllocError::NoMemory)?;
        let ptr = NonNull::new(start as *mut u8).ok_or(AllocError::NoMemory)?;
        if end > self.page_cursor {
            return Err(AllocError::NoMemory);
        }
        self.byte_cursor = end;
        self.byte_allocations += 1;
        Ok(ptr)
    }

    fn dealloc(&mut self, _pos: NonNull<u8>, _layout: Layout) {
        self.byte_allocations -= 1;
        if self.byte_allocations == 0 {
            self.byte_cursor = self.start;
        }
    }

    fn total_bytes(&self) -> usize {
        self.end - self.start
    }

    fn used_bytes(&self) -> usize {
        self.byte_cursor - self.start
    }

    fn available_bytes(&self) -> usize {
        self.page_cursor - self.byte_cursor
    }
}

impl<const SIZE: usize> PageAllocator for EarlyAllocator<SIZE> {
    const PAGE_SIZE: usize = SIZE;

    fn alloc_pages(&mut self, num_pages: usize, align_pow2: usize) -> AllocResult<usize> {
        if num_pages == 0 || !align_pow2.is_power_of_two() {
            return Err(AllocError::InvalidParam);
        }
        let size = num_pages.checked_mul(SIZE).ok_or(AllocError::NoMemory)?;
        let start = self
            .page_cursor
            .checked_sub(size)
            .ok_or(AllocError::NoMemory)?
            & !(align_pow2.max(SIZE) - 1);
        if start < self.byte_cursor {
            return Err(AllocError::NoMemory);
        }
        self.page_cursor = start;
        Ok(start)
    }

    fn dealloc_pages(&mut self, _pos: usize, _num_pages: usize) {}

    fn total_pages(&self) -> usize {
        (self.end / SIZE).saturating_sub(self.start.div_ceil(SIZE))
    }

    fn used_pages(&self) -> usize {
        self.end / SIZE - self.page_cursor / SIZE
    }

    fn available_pages(&self) -> usize {
        (self.page_cursor / SIZE).saturating_sub(self.byte_cursor.div_ceil(SIZE))
    }
}
