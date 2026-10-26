use bump_allocator::BumpAllocator;
use std::alloc::{GlobalAlloc, Layout};
use std::sync::Arc;
use std::thread;

#[test]
fn concurrent_allocations_are_aligned_and_disjoint() {
    let mut heap = vec![0u8; 128 * 1024];
    let start = heap.as_mut_ptr() as usize;
    let allocator = Arc::new(unsafe { BumpAllocator::new(start, start + heap.len()) });
    let mut addresses = thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let allocator = Arc::clone(&allocator);
                scope.spawn(move || {
                    let layout = Layout::from_size_align(17, 64).unwrap();
                    (0..64)
                        .map(|_| {
                            let pointer = unsafe { allocator.alloc(layout) };
                            assert!(!pointer.is_null());
                            pointer as usize
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    addresses.sort_unstable();
    assert_eq!(addresses.len(), 512);
    assert!(addresses.iter().all(|address| address % 64 == 0));
    assert!(addresses.windows(2).all(|pair| pair[0] + 17 <= pair[1]));
}
