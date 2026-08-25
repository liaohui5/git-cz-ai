use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

pub struct LoadingSpinner {
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl LoadingSpinner {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            handle: None,
        }
    }

    // start loading animation
    pub fn start(&mut self, message: &str) {
        if self.running.load(Ordering::SeqCst) {
            self.stop();
        }

        self.running.store(true, Ordering::SeqCst);
        let running_clone = self.running.clone();
        let message_clone = String::from(message);

        self.handle = Some(thread::spawn(move || {
            let spinner_chars = ['|', '/', '-', '\\'];
            let mut idx = 0;
            while running_clone.load(Ordering::SeqCst) {
                print!(
                    "\r{} {}",
                    spinner_chars[idx % spinner_chars.len()],
                    message_clone
                );
                io::stdout().flush().unwrap();

                thread::sleep(Duration::from_millis(80));
                idx += 1;
            }

            // when stop, clear the spinner
            // clear the spinner
            print!("\r\x1b[2K");
            io::stdout().flush().unwrap();
        }));
    }

    // stop loading animation
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }

        // clear the spinner
        print!("\r\x1b[2K");
        io::stdout().flush().unwrap();
    }
}

impl Default for LoadingSpinner {
    fn default() -> Self {
        Self::new()
    }
}
