extern crate spinlock;

use spinlock::Mutex;
use std::sync::Arc;

#[test]
fn it_locks() {
    let lock = Mutex::new(1984);
    let g = lock.lock();
    assert_eq!(*g, 1984);
}

#[test]
fn it_clones() {
    let l = Arc::new(Mutex::new(1984));
    let c = l.clone();
    let g = c.lock();
    assert_eq!(*g, 1984);
}

trait Foo {
    fn foo(&self);
}

struct X;
struct Y;

impl Foo for X {
    fn foo(&self) {
        println!("X");
    }
}

impl Foo for Y {
    fn foo(&self) {
        println!("Y");
    }
}

//compare against std::sync::Mutex
#[test]
fn std_mutex_supports_trait_objects() {
    type M<T> = std::sync::Mutex<T>;
    let mut v: Vec<Arc<M<dyn Foo>>> = Vec::new();
    let x = Arc::new(M::new(X {}));
    let y = Arc::new(M::new(Y {}));
    v.push(x);
    v.push(y);
    for i in v.iter() {
        i.lock().unwrap().foo();
    }
}

#[test]
fn it_supports_trait_objects() {
    type M<T> = Mutex<T>;
    let mut v: Vec<Arc<M<dyn Foo>>> = Vec::new();
    let x = Arc::new(M::new(X {}));
    let y = Arc::new(M::new(Y {}));
    v.push(x);
    v.push(y);
    for i in v.iter() {
        i.lock().foo();
    }
}
