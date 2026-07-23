use std::fmt;
use std::time::Duration;

/// Fréquence métier du MVP.
///
/// Tous les futurs systèmes temporels (production, construction, recherche,
/// missions) doivent progresser sur ces ticks, jamais directement sur les FPS.
pub const STRATEGIC_TICKS_PER_SECOND: u32 = 10;
pub const STRATEGIC_TICK_NANOS: u64 = 1_000_000_000_u64 / STRATEGIC_TICKS_PER_SECOND as u64;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StrategicTick(u64);

impl StrategicTick {
    pub const ZERO: Self = Self(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }

    pub const fn saturating_add(self, ticks: u64) -> Self {
        Self(self.0.saturating_add(ticks))
    }

    pub fn elapsed(self) -> Duration {
        Duration::from_nanos(self.0.saturating_mul(STRATEGIC_TICK_NANOS))
    }
}

impl fmt::Display for StrategicTick {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StrategicDuration {
    ticks: u64,
}

impl StrategicDuration {
    pub const ZERO: Self = Self::from_ticks(0);

    pub const fn from_ticks(ticks: u64) -> Self {
        Self { ticks }
    }

    pub const fn ticks(self) -> u64 {
        self.ticks
    }

    pub const fn is_zero(self) -> bool {
        self.ticks == 0
    }

    pub fn as_duration(&self) -> Duration {
        Duration::from_nanos(self.ticks.saturating_mul(STRATEGIC_TICK_NANOS))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TimeSpeed {
    Paused,
    #[default]
    X1,
    X2,
    X4,
}

impl TimeSpeed {
    pub const fn factor(self) -> u32 {
        match self {
            Self::Paused => 0,
            Self::X1 => 1,
            Self::X2 => 2,
            Self::X4 => 4,
        }
    }

    pub const fn is_paused(self) -> bool {
        matches!(self, Self::Paused)
    }
}

impl fmt::Display for TimeSpeed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Paused => formatter.write_str("pause"),
            Self::X1 => formatter.write_str("x1"),
            Self::X2 => formatter.write_str("x2"),
            Self::X4 => formatter.write_str("x4"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategicClockError {
    RemainderOutOfRange(u64),
    PausedResumeSpeed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrategicAdvance {
    pub ticks: StrategicDuration,
    pub current_tick: StrategicTick,
}

impl StrategicAdvance {
    pub const fn none(current_tick: StrategicTick) -> Self {
        Self {
            ticks: StrategicDuration::ZERO,
            current_tick,
        }
    }
}

/// Horloge mutable et sauvegardable de la partie.
///
/// `remainder_nanos` conserve la fraction de tick entre deux frames. Elle est
/// exprimée en nanosecondes de temps stratégique déjà multiplié par la vitesse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrategicClock {
    current_tick: StrategicTick,
    remainder_nanos: u64,
    speed: TimeSpeed,
    resume_speed: TimeSpeed,
}

impl StrategicClock {
    pub const fn new() -> Self {
        Self {
            current_tick: StrategicTick::ZERO,
            remainder_nanos: 0,
            speed: TimeSpeed::X1,
            resume_speed: TimeSpeed::X1,
        }
    }

    pub fn from_parts(
        current_tick: StrategicTick,
        remainder_nanos: u64,
        speed: TimeSpeed,
        resume_speed: TimeSpeed,
    ) -> Result<Self, StrategicClockError> {
        if remainder_nanos >= STRATEGIC_TICK_NANOS {
            return Err(StrategicClockError::RemainderOutOfRange(remainder_nanos));
        }
        if resume_speed.is_paused() {
            return Err(StrategicClockError::PausedResumeSpeed);
        }

        Ok(Self {
            current_tick,
            remainder_nanos,
            speed,
            resume_speed: if speed.is_paused() {
                resume_speed
            } else {
                speed
            },
        })
    }

    pub const fn current_tick(&self) -> StrategicTick {
        self.current_tick
    }

    pub const fn remainder_nanos(&self) -> u64 {
        self.remainder_nanos
    }

    pub const fn speed(&self) -> TimeSpeed {
        self.speed
    }

    pub const fn resume_speed(&self) -> TimeSpeed {
        self.resume_speed
    }

    pub fn elapsed(&self) -> Duration {
        self.current_tick.elapsed()
    }

    pub fn elapsed_seconds(&self) -> f64 {
        self.elapsed().as_secs_f64()
    }

    pub fn set_speed(&mut self, speed: TimeSpeed) -> bool {
        if self.speed == speed {
            return false;
        }

        self.speed = speed;
        if !speed.is_paused() {
            self.resume_speed = speed;
        }
        true
    }

    pub fn toggle_pause(&mut self) -> TimeSpeed {
        let next = if self.speed.is_paused() {
            self.resume_speed
        } else {
            TimeSpeed::Paused
        };
        self.set_speed(next);
        next
    }

    pub fn advance(&mut self, real_delta: Duration) -> StrategicAdvance {
        let factor = u64::from(self.speed.factor());
        if factor == 0 || real_delta.is_zero() {
            return StrategicAdvance::none(self.current_tick);
        }

        let real_nanos = real_delta.as_nanos().min(u128::from(u64::MAX)) as u64;
        let scaled_nanos = real_nanos.saturating_mul(factor);
        let total_nanos = self.remainder_nanos.saturating_add(scaled_nanos);
        let advanced_ticks = total_nanos / STRATEGIC_TICK_NANOS;

        self.remainder_nanos = total_nanos % STRATEGIC_TICK_NANOS;
        self.current_tick = self.current_tick.saturating_add(advanced_ticks);

        StrategicAdvance {
            ticks: StrategicDuration::from_ticks(advanced_ticks),
            current_tick: self.current_tick,
        }
    }
}

impl Default for StrategicClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_frames_accumulate_into_fixed_ticks() {
        let mut clock = StrategicClock::new();

        assert!(clock.advance(Duration::from_millis(40)).ticks.is_zero());
        assert!(clock.advance(Duration::from_millis(40)).ticks.is_zero());

        let result = clock.advance(Duration::from_millis(20));

        assert_eq!(result.ticks, StrategicDuration::from_ticks(1));
        assert_eq!(clock.current_tick(), StrategicTick::new(1));
        assert_eq!(clock.remainder_nanos(), 0);
    }

    #[test]
    fn pause_resumes_the_previous_speed() {
        let mut clock = StrategicClock::new();
        clock.set_speed(TimeSpeed::X4);

        assert_eq!(clock.toggle_pause(), TimeSpeed::Paused);
        assert_eq!(clock.toggle_pause(), TimeSpeed::X4);
    }

    #[test]
    fn invalid_saved_remainder_is_rejected() {
        assert_eq!(
            StrategicClock::from_parts(
                StrategicTick::ZERO,
                STRATEGIC_TICK_NANOS,
                TimeSpeed::X1,
                TimeSpeed::X1,
            ),
            Err(StrategicClockError::RemainderOutOfRange(
                STRATEGIC_TICK_NANOS
            ))
        );
    }
}
