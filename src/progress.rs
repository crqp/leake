use std::io::{self, Write};
use std::time::{Duration, Instant};

const REDRAW_INTERVAL: Duration = Duration::from_millis(100);

pub struct Progress {
    label: String,
    width: usize,
    last_draw: Option<Instant>,
    closed: bool,
}

impl Progress {
    #[must_use]
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_owned(),
            width: 28,
            last_draw: None,
            closed: false,
        }
    }

    pub fn update(&mut self, fraction: f64) {
        let clamped = fraction.clamp(0.0, 1.0);
        if self.closed {
            return;
        }
        let now = Instant::now();
        if clamped < 1.0
            && self
                .last_draw
                .is_some_and(|last| now.duration_since(last) < REDRAW_INTERVAL)
        {
            return;
        }
        self.last_draw = Some(now);
        let filled = (clamped * self.width as f64) as usize;
        let filled = filled.clamp(0, self.width);
        let percent = (clamped * 100.0) as u32;
        let percent = percent.clamp(0, 100);
        let mut stderr = io::stderr().lock();
        let _ = write!(
            stderr,
            "\r{}  [{}{}]  {percent:>3}%",
            self.label,
            "#".repeat(filled),
            "-".repeat(self.width - filled),
        );
        let _ = stderr.flush();
    }

    pub fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            let mut stderr = io::stderr().lock();
            let _ = writeln!(stderr);
            let _ = stderr.flush();
        }
    }
}
