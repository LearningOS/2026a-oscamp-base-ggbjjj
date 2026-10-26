use atomic_ordering::OnceCell;
use std::sync::Barrier;
use std::thread;

#[test]
fn racing_readers_observe_only_the_fully_published_winner() {
    for _ in 0..64 {
        let cell = OnceCell::new();
        let barrier = Barrier::new(9);
        thread::scope(|scope| {
            let writers: Vec<_> = (1..=4)
                .map(|value| {
                    let cell = &cell;
                    let barrier = &barrier;
                    scope.spawn(move || {
                        barrier.wait();
                        cell.init(value)
                    })
                })
                .collect();
            let readers: Vec<_> = (0..4)
                .map(|_| {
                    let cell = &cell;
                    let barrier = &barrier;
                    scope.spawn(move || {
                        barrier.wait();
                        loop {
                            if let Some(value) = cell.get() {
                                assert!(
                                    (1..=4).contains(&value),
                                    "unpublished placeholder was visible"
                                );
                                return value;
                            }
                            thread::yield_now();
                        }
                    })
                })
                .collect();
            barrier.wait();
            let wins = writers
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .filter(|&won| won)
                .count();
            assert_eq!(wins, 1);
            let published = cell.get().unwrap();
            for reader in readers {
                assert_eq!(reader.join().unwrap(), published);
            }
        });
    }
}
