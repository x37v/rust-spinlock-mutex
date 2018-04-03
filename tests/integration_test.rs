extern crate spinlock;

use spinlock::Mutex;

#[test]
fn it_locks() {
    let lock = Mutex::new(1984);
    let clone = lock.clone();
    let g = lock.lock();
    assert_eq!(*g, 1984);

    let tl = clone.try_lock();
    assert!(tl.is_err());
}
