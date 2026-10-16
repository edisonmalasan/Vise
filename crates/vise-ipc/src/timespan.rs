//! Strict duration parsing for the worker scan contract.
//!
//! Scan requests carry minimum and maximum title durations. The format is
//! `[-][days.]hh:mm:ss[.fraction]` with exactly two digits per clock field and
//! hours restricted to `00`–`23`; anything longer than a day must be written
//! with the day component. Parsing never panics and never silently accepts a
//! malformed value: a bad format is reported as a structured error.

use std::fmt;
use std::time::Duration;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Why a duration string was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeSpanError {
    /// The value was empty or only whitespace.
    Empty,
    /// The value did not contain the `hh:mm:ss` clock part.
    MissingClock,
    /// The value was not `[-][days.]hh:mm:ss[.fraction]`.
    Malformed,
    /// The day component was not a plain number.
    InvalidDays,
    /// A clock field was not exactly two digits.
    InvalidClockField,
    /// An hour component outside `00`–`23`.
    HourOutOfRange,
    /// A minute or second component above 59.
    FieldOutOfRange,
    /// The fractional part was present but not 1–7 digits.
    InvalidFraction,
    /// The value could not be represented as a duration.
    OutOfRange,
}

impl fmt::Display for TimeSpanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimeSpanError::Empty => write!(f, "duration is empty"),
            TimeSpanError::MissingClock => {
                write!(f, "duration must contain the hh:mm:ss clock part")
            }
            TimeSpanError::Malformed => {
                write!(
                    f,
                    "duration must use the [-][days.]hh:mm:ss[.fraction] format"
                )
            }
            TimeSpanError::InvalidDays => write!(f, "duration day component must be a number"),
            TimeSpanError::InvalidClockField => {
                write!(
                    f,
                    "duration hour, minute, and second fields must each have two digits"
                )
            }
            TimeSpanError::HourOutOfRange => {
                write!(f, "duration hours must be between 00 and 23; use the day component for longer spans")
            }
            TimeSpanError::FieldOutOfRange => {
                write!(f, "duration minutes and seconds must be between 00 and 59")
            }
            TimeSpanError::InvalidFraction => {
                write!(
                    f,
                    "duration fraction must contain between one and seven digits"
                )
            }
            TimeSpanError::OutOfRange => write!(f, "duration is too large to represent"),
        }
    }
}

impl std::error::Error for TimeSpanError {}

/// A parsed, non-negative span with millisecond resolution.
///
/// The parser accepts a leading `-`, which is how a caller expresses "no
/// lower bound" for a minimum duration; [`TimeSpan::to_std`] then yields zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct TimeSpan {
    /// True when the source value carried a leading `-`.
    pub negative: bool,
    pub days: u32,
    pub hours: u32,
    pub minutes: u32,
    pub seconds: u32,
    pub milliseconds: u32,
}

impl TimeSpan {
    /// The zero span.
    pub const ZERO: TimeSpan = TimeSpan {
        negative: false,
        days: 0,
        hours: 0,
        minutes: 0,
        seconds: 0,
        milliseconds: 0,
    };

    /// Parse a duration string in `[-][days.]hh:mm:ss[.fraction]` form.
    pub fn parse(input: &str) -> Result<TimeSpan, TimeSpanError> {
        let text = input.trim();
        if text.is_empty() {
            return Err(TimeSpanError::Empty);
        }

        let (negative, rest) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text.strip_prefix('+').unwrap_or(text)),
        };
        if rest.is_empty() {
            return Err(TimeSpanError::Malformed);
        }

        // The day separator is the first '.' that appears before the clock part;
        // a '.' after the first ':' is the fractional separator.
        let clock_start = rest.find(':').unwrap_or(rest.len());
        let (days, clock) = match rest[..clock_start].find('.') {
            Some(dot) => {
                let day_text = &rest[..dot];
                if day_text.is_empty() || !day_text.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(TimeSpanError::InvalidDays);
                }
                let days: u32 = day_text.parse().map_err(|_| TimeSpanError::InvalidDays)?;
                (days, &rest[dot + 1..])
            }
            None => (0, rest),
        };

        let (clock, fraction) = match clock.split_once('.') {
            Some((clock, fraction)) => (clock, Some(fraction)),
            None => (clock, None),
        };

        let clock_parts: Vec<&str> = clock.split(':').collect();
        if clock_parts.len() != 3 {
            return Err(if clock_parts.len() < 3 {
                TimeSpanError::MissingClock
            } else {
                TimeSpanError::Malformed
            });
        }
        let hours = parse_clock_field(clock_parts[0])?;
        let minutes = parse_clock_field(clock_parts[1])?;
        let seconds = parse_clock_field(clock_parts[2])?;
        if hours > 23 {
            return Err(TimeSpanError::HourOutOfRange);
        }
        if minutes > 59 || seconds > 59 {
            return Err(TimeSpanError::FieldOutOfRange);
        }

        let milliseconds = match fraction {
            None => 0,
            Some(digits) => {
                if digits.is_empty()
                    || digits.len() > 7
                    || !digits.bytes().all(|b| b.is_ascii_digit())
                {
                    return Err(TimeSpanError::InvalidFraction);
                }
                // Pad to the 100 ns tick resolution the format allows, then
                // round to the nearest millisecond.
                let padded = format!("{digits:0<7}");
                let ticks: u32 = padded.parse().map_err(|_| TimeSpanError::InvalidFraction)?;
                (ticks + 5_000) / 10_000
            }
        };

        Ok(TimeSpan {
            negative,
            days,
            hours,
            minutes,
            seconds,
            milliseconds,
        })
    }

    /// Whole milliseconds in this span; zero for a negated span.
    pub fn total_milliseconds(self) -> u64 {
        if self.negative {
            return 0;
        }
        (self.days as u64 * 86_400_000)
            + (self.hours as u64 * 3_600_000)
            + (self.minutes as u64 * 60_000)
            + (self.seconds as u64 * 1_000)
            + self.milliseconds as u64
    }

    /// Convert to a [`Duration`]; a negated span becomes zero.
    pub fn to_std(self) -> Duration {
        Duration::from_millis(self.total_milliseconds())
    }

    /// Build a span from a [`Duration`], rounding down to milliseconds.
    pub fn from_std(duration: Duration) -> TimeSpan {
        let total_ms = duration.as_millis().min(u64::MAX as u128) as u64;
        TimeSpan {
            negative: false,
            days: (total_ms / 86_400_000) as u32,
            hours: ((total_ms / 3_600_000) % 24) as u32,
            minutes: ((total_ms / 60_000) % 60) as u32,
            seconds: ((total_ms / 1_000) % 60) as u32,
            milliseconds: (total_ms % 1_000) as u32,
        }
    }

    /// Canonical text form, omitting a zero day component and a zero fraction.
    pub fn display(self) -> String {
        let sign = if self.negative { "-" } else { "" };
        let clock = format!("{:02}:{:02}:{:02}", self.hours, self.minutes, self.seconds);
        let base = if self.days > 0 {
            format!("{}{}.{}", sign, self.days, clock)
        } else {
            format!("{sign}{clock}")
        };
        if self.milliseconds > 0 {
            format!("{base}.{:03}", self.milliseconds)
        } else {
            base
        }
    }
}

impl fmt::Display for TimeSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display())
    }
}

impl Serialize for TimeSpan {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.display())
    }
}

impl<'de> Deserialize<'de> for TimeSpan {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<TimeSpan, D::Error> {
        let raw = String::deserialize(deserializer)?;
        TimeSpan::parse(&raw).map_err(|e| D::Error::custom(e.to_string()))
    }
}

fn parse_clock_field(field: &str) -> Result<u32, TimeSpanError> {
    if field.len() != 2 || !field.bytes().all(|b| b.is_ascii_digit()) {
        return Err(TimeSpanError::InvalidClockField);
    }
    field.parse().map_err(|_| TimeSpanError::InvalidClockField)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clock_only_forms() {
        let zero = TimeSpan::parse("00:00:00").expect("zero");
        assert_eq!(zero.total_milliseconds(), 0);
        assert_eq!(zero.display(), "00:00:00");

        let ten = TimeSpan::parse("00:00:10").expect("ten seconds");
        assert_eq!(ten.total_milliseconds(), 10_000);

        let max_day = TimeSpan::parse("23:59:59").expect("max clock");
        assert_eq!(max_day.total_milliseconds(), 86_399_000);
    }

    #[test]
    fn parses_day_components_and_fractions() {
        let day = TimeSpan::parse("1.00:00:00").expect("one day");
        assert_eq!(day.days, 1);
        assert_eq!(day.total_milliseconds(), 86_400_000);
        assert_eq!(day.display(), "1.00:00:00");

        let frac = TimeSpan::parse("00:00:01.5").expect("fraction");
        assert_eq!(frac.milliseconds, 500);

        let ticks = TimeSpan::parse("00:00:00.1234567").expect("tick precision");
        assert_eq!(ticks.milliseconds, 123);

        let rounded = TimeSpan::parse("00:00:00.0005000").expect("rounded up");
        assert_eq!(rounded.milliseconds, 1);
        let rounded_down = TimeSpan::parse("00:00:00.0004000").expect("rounded down");
        assert_eq!(rounded_down.milliseconds, 0);
    }

    #[test]
    fn negative_spans_are_accepted_as_zero() {
        let negated = TimeSpan::parse("-00:00:05").expect("negated");
        assert!(negated.negative);
        assert_eq!(negated.total_milliseconds(), 0);
        assert_eq!(negated.to_std(), Duration::ZERO);
        assert_eq!(negated.display(), "-00:00:05");
    }

    #[test]
    fn rejects_out_of_range_clock_values() {
        assert_eq!(
            TimeSpan::parse("99:59:59"),
            Err(TimeSpanError::HourOutOfRange)
        );
        assert_eq!(
            TimeSpan::parse("00:99:00"),
            Err(TimeSpanError::FieldOutOfRange)
        );
        assert_eq!(
            TimeSpan::parse("00:00:60"),
            Err(TimeSpanError::FieldOutOfRange)
        );
    }

    #[test]
    fn rejects_structurally_invalid_values() {
        assert_eq!(TimeSpan::parse(""), Err(TimeSpanError::Empty));
        assert_eq!(TimeSpan::parse("   "), Err(TimeSpanError::Empty));
        assert_eq!(TimeSpan::parse("abc"), Err(TimeSpanError::MissingClock));
        assert_eq!(TimeSpan::parse("00:00"), Err(TimeSpanError::MissingClock));
        assert_eq!(
            TimeSpan::parse("00:00:00:00"),
            Err(TimeSpanError::Malformed)
        );
        assert_eq!(
            TimeSpan::parse("0:00:00"),
            Err(TimeSpanError::InvalidClockField)
        );
        assert_eq!(
            TimeSpan::parse("00:00:0a"),
            Err(TimeSpanError::InvalidClockField)
        );
        assert_eq!(
            TimeSpan::parse("x.00:00:00"),
            Err(TimeSpanError::InvalidDays)
        );
        assert_eq!(
            TimeSpan::parse("00:00:00."),
            Err(TimeSpanError::InvalidFraction)
        );
        assert_eq!(
            TimeSpan::parse("00:00:00.12345678"),
            Err(TimeSpanError::InvalidFraction)
        );
        assert_eq!(
            TimeSpan::parse("99999999999999999999.00:00:00"),
            Err(TimeSpanError::InvalidDays)
        );
    }

    #[test]
    fn serde_round_trip_and_error_message() {
        let span = TimeSpan::parse("1.02:03:04.500").expect("parse");
        let json = serde_json::to_string(&span).expect("serialize");
        assert_eq!(json, "\"1.02:03:04.500\"");
        let back: TimeSpan = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, span);

        let err = serde_json::from_str::<TimeSpan>("\"99:59:59\"").unwrap_err();
        assert!(
            err.to_string().contains("between 00 and 23"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn duration_round_trip() {
        let span = TimeSpan::from_std(Duration::from_millis(90_061_500));
        assert_eq!(span.display(), "1.01:01:01.500");
        assert_eq!(TimeSpan::parse(&span.display()).unwrap(), span);
    }
}
