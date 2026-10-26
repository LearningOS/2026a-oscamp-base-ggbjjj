use free_list_allocator::FreeListAllocator;
use std::alloc::{GlobalAlloc, Layout};
use std::sync::Arc;
use std::thread;

#[test]
fn production_free_list_supports_concurrent_reclamation() {
    // Integration tests compile the real no_std library, without its cfg(test) mutex.
    let mut heap = vec![0u8; 256 * 1024];
    let start = heap.as_mut_ptr() as usize;
    let allocator = Arc::new(unsafe { FreeListAllocator::new(start, start + heap.len()) });
    thread::scope(|scope| {
        let handles: Vec<_> = (1..=8)
            .map(|id| {
                let allocator = Arc::clone(&allocator);
                scope.spawn(move || {
                    let layout = Layout::from_size_align(64, 64).unwrap();
                    for _ in 0..200 {
                        let pointer = unsafe { allocator.alloc(layout) };
                        assert!(!pointer.is_null());
                        assert_eq!(pointer as usize % layout.align(), 0);
                        // SAFETY: This successful allocation is exclusively owned until dealloc.
                        let bytes =
                            unsafe { std::slice::from_raw_parts_mut(pointer, layout.size()) };
                        bytes.fill(id);
                        thread::yield_now();
                        assert!(bytes.iter().all(|&value| value == id));
                        unsafe { allocator.dealloc(pointer, layout) };
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
    });
}
