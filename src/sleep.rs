use futures::task::{Waker, Context, Poll};
use futures::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use rusty_pool::ThreadPool;

static mut WAITING: Option<Mutex<Vec<Sleep>>> = None;

pub fn start(pool : ThreadPool, millis : u64) {
    thread::spawn(move || {
        let mutex;
        unsafe {
            if WAITING.is_none() {
                WAITING = Some(Mutex::new(Vec::new()));
            }
            mutex = WAITING.as_ref().unwrap()
        }
        loop {
            let mut guard = mutex.lock().unwrap();
            for s in guard.iter_mut() {
                let waker = s.waker.as_ref().unwrap().clone();
                pool.execute(move || {
                    waker.wake();
                });
                thread::sleep(Duration::from_millis(millis));
            }
            guard.clear();
            drop(guard);
        }
    });
}

#[derive(Clone)]
pub struct Sleep {
    done: bool,
    waker: Option<Waker>,
}

impl Sleep {
    pub fn new() -> Sleep {
        Sleep { done: false, waker: None }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.done {
            Poll::Ready(())
        } else {
            self.as_mut().waker = Some(cx.waker().clone());
            let mutex = unsafe { WAITING.as_ref() }.unwrap();
            let mut guard = mutex.lock().unwrap();

            guard.push(self.clone());
            self.done = true;

            Poll::Pending
        }
    }
}