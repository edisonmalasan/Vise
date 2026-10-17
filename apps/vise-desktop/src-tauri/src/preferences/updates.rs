//! The updates tab: how often a newer release is looked for, and which is followed.
//!
//! Two preferences, and neither of them is wired to a network call because this
//! build makes none: there is no update checker to hand a frequency to, and
//! offering a "check now" that quietly does nothing would be the one thing worse
//! than not offering it.
//!
//! What the preferences *do* reach is the schedule. The frequency is resolved
//! against a date the window holds, and the result is shown: when the next check
//! falls, and what "never" means in days. That is a real answer produced by the
//! stored value, not a promise, and the control that would actually fetch
//! something is closed with the reason naming what is missing.
//!
//! The channel is resolved the same way: it says which releases the application
//! would follow, and it is the same channel every answer about updating is read
//! from, so the one stored value is what a future checker has to honour.

use serde::{Deserialize, Serialize};

use crate::preferences::{
    want_one_of, ActionViewModel, ChoiceGroupViewModel, FactViewModel, PreferenceTabViewModel,
    TextViewModel,
};
use crate::tabs::common::ChoiceOption;

/// Field identifier for the check frequency.
pub const FREQUENCY: &str = "updates.frequency";
/// Field identifier for the release channel.
pub const CHANNEL: &str = "updates.channel";
/// Field identifier for where the release feed is.
pub const FEED_URL: &str = "updates.feed_url";

/// How often a newer release is looked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CheckFrequency {
    /// Not at all.
    Never,
    /// Once a day.
    Daily,
    /// Once a week.
    #[default]
    Weekly,
    /// Once a month.
    Monthly,
}

impl CheckFrequency {
    /// Every choice, in the order the picker lists them.
    pub const ALL: [CheckFrequency; 4] = [
        CheckFrequency::Never,
        CheckFrequency::Daily,
        CheckFrequency::Weekly,
        CheckFrequency::Monthly,
    ];

    /// Stable identifier the interface layer sends back.
    pub fn id(self) -> &'static str {
        match self {
            CheckFrequency::Never => "never",
            CheckFrequency::Daily => "daily",
            CheckFrequency::Weekly => "weekly",
            CheckFrequency::Monthly => "monthly",
        }
    }

    /// Label on the option.
    pub fn label(self) -> &'static str {
        match self {
            CheckFrequency::Never => "Never",
            CheckFrequency::Daily => "Every day",
            CheckFrequency::Weekly => "Every week",
            CheckFrequency::Monthly => "Every month",
        }
    }

    /// Days between checks, or nothing when there are none.
    pub fn days(self) -> Option<u32> {
        match self {
            CheckFrequency::Never => None,
            CheckFrequency::Daily => Some(1),
            CheckFrequency::Weekly => Some(7),
            CheckFrequency::Monthly => Some(30),
        }
    }

    /// What choosing this means, once a date is known.
    pub fn next_check_from(self, checked_on: (u32, u32, u32)) -> Option<(u32, u32, u32)> {
        let days = self.days()?;
        Some(add_days(checked_on, days as i64))
    }

    /// How often this is, in the words a reader would use.
    ///
    /// A count of days and the same sentence for every one of them reads like a
    /// database column, so the number is spelled: "once a day" rather than "once
    /// every 1 day(s)".
    pub fn interval_label(self) -> String {
        match self {
            CheckFrequency::Never => "No release is looked for.".to_string(),
            CheckFrequency::Daily => "That is once a day.".to_string(),
            CheckFrequency::Weekly => "That is once every seven days.".to_string(),
            CheckFrequency::Monthly => "That is once every thirty days.".to_string(),
        }
    }

    /// Read a frequency the interface layer sent, or say why it was refused.
    pub fn read(value: &serde_json::Value) -> Result<Self, String> {
        let typed = want_one_of(
            FREQUENCY,
            value,
            &[
                (CheckFrequency::Never.id(), CheckFrequency::Never.label()),
                (CheckFrequency::Daily.id(), CheckFrequency::Daily.label()),
                (CheckFrequency::Weekly.id(), CheckFrequency::Weekly.label()),
                (
                    CheckFrequency::Monthly.id(),
                    CheckFrequency::Monthly.label(),
                ),
            ],
        )?;
        CheckFrequency::ALL
            .into_iter()
            .find(|frequency| frequency.id() == typed)
            .ok_or_else(|| format!("'{typed}' is not a check frequency this build offers."))
    }
}

/// Which releases the application follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UpdateChannel {
    /// Releases everybody gets.
    #[default]
    Stable,
    /// Releases that are being tried out.
    Preview,
}

impl UpdateChannel {
    /// Every channel, in the order the picker lists them.
    pub const ALL: [UpdateChannel; 2] = [UpdateChannel::Stable, UpdateChannel::Preview];

    /// Stable identifier the interface layer sends back.
    pub fn id(self) -> &'static str {
        match self {
            UpdateChannel::Stable => "stable",
            UpdateChannel::Preview => "preview",
        }
    }

    /// Label on the option.
    pub fn label(self) -> &'static str {
        match self {
            UpdateChannel::Stable => "Stable releases",
            UpdateChannel::Preview => "Preview releases",
        }
    }

    /// What choosing this means.
    pub fn hint(self) -> &'static str {
        match self {
            UpdateChannel::Stable => "Only releases that have been tried out on other machines.",
            UpdateChannel::Preview => {
                "Also releases that are still being tried out, which may behave differently."
            }
        }
    }

    /// Read a channel the interface layer sent, or say why it was refused.
    pub fn read(value: &serde_json::Value) -> Result<Self, String> {
        let typed = want_one_of(
            CHANNEL,
            value,
            &[
                (UpdateChannel::Stable.id(), UpdateChannel::Stable.label()),
                (UpdateChannel::Preview.id(), UpdateChannel::Preview.label()),
            ],
        )?;
        UpdateChannel::ALL
            .into_iter()
            .find(|channel| channel.id() == typed)
            .ok_or_else(|| format!("'{typed}' is not a release channel this build follows."))
    }
}

/// The update preferences, as they are stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateSettings {
    /// How often a newer release is looked for.
    pub frequency: CheckFrequency,
    /// Which releases are followed.
    pub channel: UpdateChannel,
    /// Where the release feed is, when one has been configured.
    ///
    /// Empty by default and refused unless it is an https address: a build with no
    /// feed contacts nobody, which is the only safe answer for a machine that has
    /// never been asked about this.
    pub feed_url: String,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdateSettings {
    /// The preferences a machine that has never chosen anything has.
    ///
    /// A weekly look on the stable channel: often enough that a security fix is
    /// not months away, rarely enough that an application that opens every day
    /// is not asking the network every day for nothing. No feed, because a build
    /// that invented one would be a build contacting a server nobody chose.
    pub fn new() -> Self {
        Self {
            frequency: CheckFrequency::Weekly,
            channel: UpdateChannel::Stable,
            feed_url: String::new(),
        }
    }

    /// Whether a feed has been configured at all.
    pub fn has_feed(&self) -> bool {
        !self.feed_url.trim().is_empty()
    }
}

/// Read a feed address, refusing one this build will not fetch.
///
/// Refused here rather than at the check, because the whole point of the rule is
/// that a control cannot store an answer nothing will act on.
pub fn read_feed(value: &serde_json::Value) -> Result<String, String> {
    let text = crate::preferences::want_text(FEED_URL, value)?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    if !crate::system::updates::is_allowed_url(trimmed) {
        return Err(format!(
            "A release feed has to be an https address, not '{trimmed}'."
        ));
    }
    Ok(trimmed.to_string())
}

/// The problems these preferences have.
///
/// Both are closed enumerations, so a record holding a value this build cannot
/// use was already refused as unreadable when it was read. The rule is kept
/// because the tab builds its problem list from it and because it is where the
/// next rule belongs.
pub fn validate(_settings: &UpdateSettings) -> Vec<String> {
    Vec::new()
}

/// Add `days` to a `(year, month, day)` date, in the proleptic Gregorian
/// calendar.
///
/// The arithmetic is done here rather than taken from the clock so the schedule
/// can be stated and tested: the tab says when the next check falls, and that
/// sentence has to be the same on every machine, not only on one whose clock and
/// time zone agree with the reader's.
pub fn add_days(date: (u32, u32, u32), days: i64) -> (u32, u32, u32) {
    let (year, month, day) = date;
    let days_in_month = |year: i64, month: u32| -> u32 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if is_leap_year(year) => 29,
            2 => 28,
            _ => 30,
        }
    };
    let mut year = year as i64;
    let mut month = month;
    let mut day = day as i64;
    day += days;
    loop {
        let length = days_in_month(year, month) as i64;
        if day <= length {
            break;
        }
        day -= length;
        month += 1;
        if month > 12 {
            month = 1;
            year += 1;
        }
    }
    if day < 1 {
        day = 1;
    }
    (year.max(0) as u32, month, day as u32)
}

/// Whether a year is a leap year in the proleptic Gregorian calendar.
pub fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// A date, written the way the page shows it.
pub fn date_label(date: (u32, u32, u32)) -> String {
    format!("{:04}-{:02}-{:02}", date.0, date.1, date.2)
}

/// Today's date, as `(year, month, day)`.
///
/// The check schedule is shown as a date rather than as an interval because a
/// date is the answer to "when will it look", and an interval is only the
/// arithmetic behind it. The conversion is here rather than left to a library so
/// the sentence on the tab is the same on every machine: a clock with a time zone
/// the reader is not in would otherwise print a different day to the same person
/// on two sides of a border.
pub fn today() -> (u32, u32, u32) {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs() as i64)
        .unwrap_or(0);
    civil_from_days(seconds.div_euclid(86_400))
}

/// The `(year, month, day)` for a count of days since 1970-01-01.
///
/// The civil calendar is the one every platform this builds for already uses for
/// dates, and this is the standard days-to-civil conversion, so the answer does
/// not depend on leap-year rules being spelled out a second way somewhere else.
pub fn civil_from_days(days: i64) -> (u32, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    (
        (year + i64::from(month <= 2)) as u32,
        month as u32,
        day as u32,
    )
}

/// Why the control that would fetch a release is closed when nothing has been
/// configured.
///
/// Named rather than hidden, because a preferences tab that stops halfway down its
/// list reads as an oversight, and a reason turns it into an answer.
pub const NO_FEED_REASON: &str =
    "No release feed is configured, so there is nothing to look at. The frequency and the channel are stored and are what a checker would read.";

/// Why the control is closed when this build cannot fetch anything.
///
/// Two answers rather than one: a build with no way to reach the network and a
/// build whose network tool is not installed are different problems, and a user who
/// installs the tool should be told that rather than told the build is incapable.
pub fn no_source_reason(source: &crate::system::updates::CurlSource) -> String {
    format!(
        "{}, and it is {} on this machine, so there is nothing to check yet.",
        crate::system::updates::AppcastSource::describe(source),
        if source.installed() {
            "available"
        } else {
            "not installed"
        }
    )
}

/// Build the updates tab.
pub fn tab_view(
    settings: &UpdateSettings,
    today: (u32, u32, u32),
    last: &crate::system::updates::UpdateState,
) -> PreferenceTabViewModel {
    let mut tab = PreferenceTabViewModel::empty(
        "updates",
        "Updates",
        "How often a newer release is looked for, and which one is followed.",
    );
    let interval = settings.frequency.interval_label();
    tab.choices.push(
        ChoiceGroupViewModel::build(
            FREQUENCY,
            "Check for updates",
            "How long between looks for a newer release.",
            settings.frequency.id(),
            &CheckFrequency::ALL
                .iter()
                .map(|frequency| ChoiceOption::new(frequency.id(), frequency.label()))
                .collect::<Vec<_>>(),
            true,
            "",
        )
        .with_hint(&interval),
    );
    tab.choices.push(
        ChoiceGroupViewModel::build(
            CHANNEL,
            "Update channel",
            "Which releases are followed.",
            settings.channel.id(),
            &UpdateChannel::ALL
                .iter()
                .map(|channel| ChoiceOption::new(channel.id(), channel.label()))
                .collect::<Vec<_>>(),
            true,
            "",
        )
        .with_hint(settings.channel.hint()),
    );
    // The read-out the schedule is resolved into: a number of days is a promise
    // and a date is an answer, so both are shown, and neither is a field —
    // typing a number of days here would be a second answer to the question the
    // picker above has already answered.
    let days = settings.frequency.days();
    tab.facts.push(match days {
        Some(days) => {
            FactViewModel::present("updatesInterval", "Days between checks", format!("{days}"))
        }
        None => FactViewModel::unknown(
            "updatesInterval",
            "Days between checks",
            "The frequency is never, so there is no interval.",
        ),
    });
    let next = settings.frequency.next_check_from(today);
    tab.facts.push(match next {
        Some(date) => FactViewModel::present("updatesNextCheck", "Next check", date_label(date)),
        None => FactViewModel::unknown(
            "updatesNextCheck",
            "Next check",
            "The frequency is never, so no release is looked for.",
        ),
    });
    // The feed is the only address this build will ever contact, so it is a field
    // the user types rather than one this build ships: a build that invented an
    // address would be a build contacting a server nobody chose.
    tab.texts.push(
        TextViewModel::new(
            "updatesFeedUrl",
            "Release feed",
            &settings.feed_url,
            "https://example.invalid/vise/appcast.xml",
        )
        .with_hint(
            "Where the list of releases is fetched from. Only https is accepted, and an empty value means nothing is contacted at all.",
        ),
    );
    tab.facts.push(match settings.has_feed() {
        true => FactViewModel::present(
            "updatesFeed",
            "Release feed",
            settings.feed_url.trim().to_string(),
        ),
        false => FactViewModel::unknown(
            "updatesFeed",
            "Release feed",
            "no feed is configured, so no release is looked for",
        ),
    });
    tab.facts.push(match &last.outcome {
        Some(_) if !last.summary.is_empty() => {
            FactViewModel::present("updatesLast", "Last check", last.summary.clone())
        }
        _ => FactViewModel::unknown(
            "updatesLast",
            "Last check",
            "nothing has been looked for yet",
        ),
    });
    // The control is on offer only when there is somewhere to look and this build
    // can reach it: a button that is pressed and does nothing is the one thing
    // worse than a button that is not there.
    let source = crate::system::updates::CurlSource::default();
    let mut check = ActionViewModel::new(
        "preferencesCheckNow",
        "Look for a release now",
        "Fetches the release feed and says what it offers. Nothing is downloaded and nothing is installed.",
    );
    if !settings.has_feed() {
        check = check.blocked(NO_FEED_REASON);
    } else if !crate::system::updates::AppcastSource::available(&source) {
        check = check.blocked(no_source_reason(&source));
    }
    tab.actions.push(check);
    tab.problems = validate(settings);
    tab.ready = tab.problems.is_empty();
    tab
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(raw: &str) -> serde_json::Value {
        serde_json::from_str(raw).expect("json")
    }

    #[test]
    fn the_defaults_are_the_ones_a_machine_that_has_chosen_nothing_has() {
        let settings = UpdateSettings::new();
        assert_eq!(settings.frequency, CheckFrequency::Weekly);
        assert_eq!(settings.channel, UpdateChannel::Stable);
        assert_eq!(settings, UpdateSettings::default());
        assert_eq!(settings.frequency.days(), Some(7));
    }

    #[test]
    fn every_frequency_resolves_to_a_number_of_days() {
        assert_eq!(CheckFrequency::Never.days(), None);
        assert_eq!(CheckFrequency::Daily.days(), Some(1));
        assert_eq!(CheckFrequency::Weekly.days(), Some(7));
        assert_eq!(CheckFrequency::Monthly.days(), Some(30));
        // Every choice round-trips through the name the page sends.
        for frequency in CheckFrequency::ALL {
            assert_eq!(
                CheckFrequency::read(&value(&format!("\"{}\"", frequency.id())))
                    .expect("round trip"),
                frequency
            );
        }
        let error = CheckFrequency::read(&value("\"hourly\"")).expect_err("unknown");
        assert!(error.contains("hourly"), "{error}");
        assert!(error.contains("weekly"), "{error}");
    }

    #[test]
    fn the_schedule_is_a_date_and_the_date_is_right() {
        // The arithmetic is here rather than taken from the clock so the sentence
        // on the tab is the same everywhere, and so it can be pinned.
        assert_eq!(
            CheckFrequency::Daily.next_check_from((2026, 10, 17)),
            Some((2026, 10, 18))
        );
        assert_eq!(
            CheckFrequency::Weekly.next_check_from((2026, 10, 17)),
            Some((2026, 10, 24))
        );
        assert_eq!(
            CheckFrequency::Monthly.next_check_from((2026, 10, 17)),
            Some((2026, 11, 16))
        );
        assert_eq!(CheckFrequency::Never.next_check_from((2026, 10, 17)), None);
    }

    #[test]
    fn the_date_arithmetic_crosses_months_years_and_leap_days() {
        assert_eq!(add_days((2026, 10, 31), 1), (2026, 11, 1));
        assert_eq!(add_days((2026, 12, 31), 1), (2027, 1, 1));
        assert_eq!(add_days((2026, 2, 28), 1), (2026, 3, 1));
        assert_eq!(
            add_days((2028, 2, 28), 1),
            (2028, 2, 29),
            "2028 is a leap year"
        );
        assert_eq!(
            add_days((2026, 1, 1), -1),
            (2026, 1, 1),
            "it never goes below one"
        );
        assert!(is_leap_year(2028));
        assert!(!is_leap_year(2026));
        assert!(
            !is_leap_year(1900),
            "a century that is not divisible by 400"
        );
        assert!(is_leap_year(2000));
        assert_eq!(date_label((2026, 1, 5)), "2026-01-05");
    }

    #[test]
    fn a_day_count_becomes_the_date_everybody_writes() {
        // Pinned against dates rather than against the clock, so this stays right
        // whatever the machine running it thinks the time is.
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(1), (1970, 1, 2));
        assert_eq!(civil_from_days(59), (1970, 3, 1));
        // 2026-10-17 is a Saturday; the day of the week is not shown, so what is
        // pinned is that this conversion agrees with the date arithmetic.
        let (year, month, day) = civil_from_days(20_743);
        assert_eq!((year, month, day), (2026, 10, 17));
        assert_eq!(add_days((year, month, day), 7), (2026, 10, 24));
        // The conversion is total: a day before the epoch still resolves.
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        let (y, m, d) = today();
        assert!((1970..=9999).contains(&y), "the window reports a real year");
        assert!((1..=12).contains(&m) && (1..=31).contains(&d));
    }

    #[test]
    fn every_channel_round_trips_and_says_what_it_follows() {
        for channel in UpdateChannel::ALL {
            assert_eq!(
                UpdateChannel::read(&value(&format!("\"{}\"", channel.id()))).expect("round trip"),
                channel
            );
            assert!(channel.hint().ends_with('.'), "{}", channel.hint());
        }
        assert!(UpdateChannel::read(&value("\"nightly\"")).is_err());
    }

    #[test]
    fn the_control_that_would_fetch_something_is_closed_with_the_reason() {
        // Storing a frequency nobody reads would be the defect this closes
        // against, so the reason says what is missing rather than apologising.
        let view = tab_view(
            &UpdateSettings::new(),
            (2026, 10, 17),
            &crate::system::updates::UpdateState::new(),
        );
        let control = &view.actions[0];
        assert_eq!(control.id, "preferencesCheckNow");
        assert!(!control.available);
        assert!(
            control.reason.contains("release feed"),
            "{}",
            control.reason
        );
        // The frequency is still resolved into something the page can show.
        let interval = view
            .facts
            .iter()
            .find(|fact| fact.id == "updatesInterval")
            .expect("the interval");
        assert_eq!(interval.value, "7");
        assert!(interval.available);
        let next = view
            .facts
            .iter()
            .find(|fact| fact.id == "updatesNextCheck")
            .expect("the next check");
        assert_eq!(
            next.value, "2026-10-24",
            "the stored frequency becomes a date"
        );
        assert!(view.numbers.is_empty(), "{:?}", view.numbers);
        // The one text field is the feed: everything else on this tab is a choice or
        // a read-out, because those are all this build decides for itself.
        assert_eq!(view.texts.len(), 1, "{:?}", view.texts);
        assert_eq!(view.texts[0].id, "updatesFeedUrl");
        assert_eq!(view.texts[0].text, "", "no feed by default");
        assert!(view.ready, "{:?}", view.problems);
    }

    #[test]
    fn a_frequency_of_never_resolves_to_no_interval_and_no_date() {
        let settings = UpdateSettings {
            frequency: CheckFrequency::Never,
            ..UpdateSettings::new()
        };
        let view = tab_view(
            &settings,
            (2026, 10, 17),
            &crate::system::updates::UpdateState::new(),
        );
        assert!(
            !view.facts[0].available,
            "no interval when nothing is checked"
        );
        assert!(
            view.facts[0].reason.contains("no interval"),
            "{}",
            view.facts[0].reason
        );
        assert!(
            !view.facts[1].available,
            "a frequency of never has no next check"
        );
        assert!(
            view.facts[1].reason.contains("never"),
            "{}",
            view.facts[1].reason
        );
        assert!(
            view.choices[0].hint.contains("No release"),
            "{}",
            view.choices[0].hint
        );
        assert_eq!(
            CheckFrequency::Never.interval_label(),
            "No release is looked for."
        );
    }

    #[test]
    fn the_interval_is_spelled_out_rather_than_counted() {
        assert_eq!(
            CheckFrequency::Daily.interval_label(),
            "That is once a day."
        );
        assert_eq!(
            CheckFrequency::Weekly.interval_label(),
            "That is once every seven days."
        );
        assert_eq!(
            CheckFrequency::Monthly.interval_label(),
            "That is once every thirty days."
        );
    }
}
