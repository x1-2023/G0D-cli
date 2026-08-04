use std::io::{IsTerminal, Write};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::JoinHandle;
use std::time::Duration;

pub struct StatusIndicator {
    running: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    animated: bool,
}

impl StatusIndicator {
    pub fn start(label: &str) -> Self {
        let animated = std::io::stderr().is_terminal();
        let running = Arc::new(AtomicBool::new(true));
        let worker_running = Arc::clone(&running);
        let label = label.to_string();

        let worker = std::thread::spawn(move || {
            if !animated {
                eprintln!("{label}...");
                return;
            }
            let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let mut frame = 0usize;
            while worker_running.load(Ordering::Relaxed) {
                eprint!("\r{} {}", frames[frame % frames.len()], label);
                let _ = std::io::stderr().flush();
                frame += 1;
                std::thread::sleep(Duration::from_millis(80));
            }
            eprint!("\r\x1b[2K");
            let _ = std::io::stderr().flush();
        });

        Self {
            running,
            worker: Some(worker),
            animated,
        }
    }

    pub fn stop(mut self) {
        self.finish();
    }

    fn finish(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        if self.animated {
            eprint!("\r\x1b[2K");
            let _ = std::io::stderr().flush();
        }
    }
}

impl Drop for StatusIndicator {
    fn drop(&mut self) {
        self.finish();
    }
}
