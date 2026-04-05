//! Heap allocator initialization (isolated unsafe).
//!
//! `embedded-alloc` 0.6 only provides `unsafe fn init()`. This module
//! confines that single unsafe call so the rest of the crate can use
//! `#![forbid(unsafe_code)]`.

#[allow(unsafe_code)]
pub fn init() {
    const HEAP_SIZE: usize = 8192;
    static mut HEAP_MEM: [core::mem::MaybeUninit<u8>; HEAP_SIZE] =
        [core::mem::MaybeUninit::uninit(); HEAP_SIZE];
    // SAFETY: called once at startup before any allocation. HEAP_MEM is a
    // module-private static only touched here.
    #[allow(static_mut_refs)]
    unsafe {
        super::HEAP.init(HEAP_MEM.as_ptr() as usize, HEAP_SIZE);
    }
}
