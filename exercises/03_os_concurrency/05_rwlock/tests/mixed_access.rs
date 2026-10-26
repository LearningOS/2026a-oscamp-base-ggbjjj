use rwlock::RwLock;
use std::sync::Arc;
use std::thread;

#[test]
fn mixed_readers_and_writers_never_observe_partial_updates() {
    let lock = Arc::new(RwLock::new((0u64, 0u64)));
    thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..5 {
            let lock = Arc::clone(&lock);
            handles.push(scope.spawn(move || {
                for _ in 0..200 {
                    let mut guard = lock.write();
                    guard.0 += 1;
                    thread::yield_now();
                    guard.1 = guard.0;
                }
            }));
        }
        for _ in 0..4 {
            let lock = Arc::clone(&lock);
            handles.push(scope.spawn(move || {
                for _ in 0..400 {
                    let guard = lock.read();
                    assert_eq!(guard.0, guard.1);
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
    });
    assert_eq!(*lock.read(), (1000, 1000));
}
