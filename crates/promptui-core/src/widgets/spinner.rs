use std::time::{Duration, Instant};

/// A set of frames that define a spinner animation.
#[derive(Debug, Clone)]
pub struct SpinnerStyle {
    /// The display frames cycled in order.
    pub frames: &'static [&'static str],
    /// Time between frame advances.
    pub interval: Duration,
}

impl SpinnerStyle {
    /// Moon waxing and waning. Each frame is emoji + trailing space for alignment.
    pub const MOON_PHASES: Self = Self {
        frames: &["🌑 ", "🌒 ", "🌓 ", "🌔 ", "🌕 ", "🌖 ", "🌗 ", "🌘 "],
        interval: Duration::from_millis(150),
    };

    /// A dot that expands and contracts. 1 cell wide.
    pub const BREATHING_DOT: Self = Self {
        frames: &["·", "∙", "•", "●", "◉", "●", "•", "∙"],
        interval: Duration::from_millis(120),
    };

    /// A sine wave traveling across 3 braille cells. 3 cells wide.
    ///
    /// Uses 4 vertical dot positions (⡀ ⠄ ⠂ ⠁) with 120° phase
    /// offset between adjacent cells.
    pub const WAVE_RIPPLE: Self = Self {
        frames: &["⡀⠂⠂", "⠄⠁⠄", "⠂⠂⡀", "⠁⠄⠄", "⠂⡀⠂", "⠄⠄⠁"],
        interval: Duration::from_millis(100),
    };
}

/// A spinner animation that cycles through frames at a fixed interval.
pub struct Spinner {
    style: SpinnerStyle,
    frame: usize,
    last_tick: Instant,
    active: bool,
}

impl Spinner {
    pub fn new(style: SpinnerStyle) -> Self {
        Self {
            style,
            frame: 0,
            last_tick: Instant::now(),
            active: false,
        }
    }

    /// Start the spinner, resetting to frame 0.
    pub fn start(&mut self) {
        self.active = true;
        self.frame = 0;
        self.last_tick = Instant::now();
    }

    /// Stop the spinner.
    pub fn stop(&mut self) {
        self.active = false;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Advance the frame if enough time has passed. Returns true if the frame changed.
    pub fn tick(&mut self) -> bool {
        if !self.active {
            return false;
        }
        let now = Instant::now();
        if now.duration_since(self.last_tick) >= self.style.interval {
            self.frame = (self.frame + 1) % self.style.frames.len();
            self.last_tick = now;
            true
        } else {
            false
        }
    }

    /// Current frame text. Returns empty string when inactive.
    pub fn frame(&self) -> &str {
        if self.active {
            self.style.frames[self.frame]
        } else {
            ""
        }
    }

    /// Switch to a different spinner style, resetting the animation.
    pub fn set_style(&mut self, style: SpinnerStyle) {
        self.style = style;
        self.frame = 0;
        self.last_tick = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breathing_dot_cycles() {
        let mut s = Spinner::new(SpinnerStyle::BREATHING_DOT);
        s.start();
        assert_eq!(s.frame(), "·");
        // Force advance by setting last_tick in the past
        s.last_tick = Instant::now() - Duration::from_millis(200);
        assert!(s.tick());
        assert_eq!(s.frame(), "∙");
    }

    #[test]
    fn inactive_spinner_returns_empty() {
        let s = Spinner::new(SpinnerStyle::BREATHING_DOT);
        assert!(!s.is_active());
        assert_eq!(s.frame(), "");
    }

    #[test]
    fn tick_does_nothing_when_inactive() {
        let mut s = Spinner::new(SpinnerStyle::BREATHING_DOT);
        s.last_tick = Instant::now() - Duration::from_millis(200);
        assert!(!s.tick());
    }

    #[test]
    fn tick_does_not_advance_too_early() {
        let mut s = Spinner::new(SpinnerStyle::BREATHING_DOT);
        s.start();
        // Just started, interval hasn't elapsed
        assert!(!s.tick());
        assert_eq!(s.frame(), "·");
    }

    #[test]
    fn start_resets_frame() {
        let mut s = Spinner::new(SpinnerStyle::WAVE_RIPPLE);
        s.start();
        s.last_tick = Instant::now() - Duration::from_millis(200);
        s.tick();
        assert_eq!(s.frame(), "⠄⠁⠄");
        s.start();
        assert_eq!(s.frame(), "⡀⠂⠂");
    }

    #[test]
    fn wave_ripple_has_six_frames() {
        assert_eq!(SpinnerStyle::WAVE_RIPPLE.frames.len(), 6);
    }

    #[test]
    fn moon_phases_has_eight_frames() {
        assert_eq!(SpinnerStyle::MOON_PHASES.frames.len(), 8);
    }

    #[test]
    fn set_style_resets() {
        let mut s = Spinner::new(SpinnerStyle::BREATHING_DOT);
        s.start();
        s.last_tick = Instant::now() - Duration::from_millis(200);
        s.tick();
        s.set_style(SpinnerStyle::MOON_PHASES);
        assert_eq!(s.frame(), "🌑 ");
    }
}
