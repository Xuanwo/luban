use std::process::Child;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub(super) fn spawn_cancel_killer(
    child: Arc<std::sync::Mutex<Child>>,
    cancel: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        while !finished.load(Ordering::SeqCst) && !cancel.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(25));
        }
        if cancel.load(Ordering::SeqCst)
            && let Ok(mut child) = child.lock()
        {
            let _ = child.kill();
        }
    })
}
