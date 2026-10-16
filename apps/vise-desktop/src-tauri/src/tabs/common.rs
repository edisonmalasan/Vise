//! Shared control primitives for the configuration tabs.
//!
//! Every tab is built from the same small vocabulary: a single choice, a
//! switch, a number, a line of text. Keeping one shape for each means the page
//! has one renderer per kind instead of one per tab, and it means an
//! unavailable control always carries the same two extra fields — `available`
//! and `reason` — so nothing is ever shown greyed out without an explanation.

use serde::{Deserialize, Serialize};

/// One option inside a single-choice control.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChoiceOption {
    /// Stable identifier the interface layer sends back.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// Whether this option can be used right now.
    pub available: bool,
    /// Why it cannot, when it cannot.
    pub reason: String,
}

impl ChoiceOption {
    /// An option that is always on offer.
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            available: true,
            reason: String::new(),
        }
    }

    /// An option that is on offer but not for the current selection.
    pub fn taken(id: &str, label: &str, reason: impl Into<String>) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            available: false,
            reason: reason.into(),
        }
    }

    /// The option, marked unavailable with `reason`.
    pub fn blocked(mut self, reason: impl Into<String>) -> Self {
        self.available = false;
        self.reason = reason.into();
        self
    }
}

/// A single-choice control as the window holds it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ChoiceGroup {
    /// Every option, in the order the picker lists them.
    pub options: Vec<ChoiceOption>,
}

impl ChoiceGroup {
    /// A group built from the identifiers and labels given.
    pub fn of<I>(entries: I) -> Self
    where
        I: IntoIterator<Item = (&'static str, &'static str)>,
    {
        let options = entries
            .into_iter()
            .map(|(id, label)| ChoiceOption::new(id, label))
            .collect();
        Self { options }
    }
}

/// One option, as the picker shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChoiceOptionViewModel {
    /// Stable identifier the interface layer sends back.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// Whether the option can be picked right now.
    pub available: bool,
    /// Why it cannot, when it cannot.
    pub reason: String,
    /// Whether this is the chosen option.
    pub selected: bool,
}

/// A single-choice control as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChoiceGroupViewModel {
    /// Element identifier of the control.
    pub id: String,
    /// Label above the control.
    pub label: String,
    /// One line explaining what the control decides.
    pub hint: String,
    /// Whether the whole control can be used.
    pub available: bool,
    /// Why the whole control cannot be used, when it cannot.
    pub reason: String,
    /// Chosen option identifier.
    pub selected: String,
    /// Every option with its availability.
    pub options: Vec<ChoiceOptionViewModel>,
}

impl ChoiceGroupViewModel {
    /// The control, unavailable with a reason the user can act on.
    pub fn blocked(mut self, reason: impl Into<String>) -> Self {
        self.available = false;
        self.reason = reason.into();
        self
    }
}

impl ChoiceGroupViewModel {
    /// Build the rendered control from the held group.
    pub fn build(
        id: &str,
        label: &str,
        hint: &str,
        selected: &str,
        options: &[ChoiceOption],
        available: bool,
        reason: &str,
    ) -> Self {
        let selected = selected.trim().to_string();
        Self {
            id: id.to_string(),
            label: label.to_string(),
            hint: hint.to_string(),
            available,
            reason: reason.to_string(),
            selected: selected.clone(),
            options: options
                .iter()
                .map(|option| ChoiceOptionViewModel {
                    id: option.id.clone(),
                    label: option.label.clone(),
                    available: option.available,
                    reason: option.reason.clone(),
                    selected: option.id == selected,
                })
                .collect(),
        }
    }
}

/// A switch as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleViewModel {
    /// Element identifier of the control.
    pub id: String,
    /// Label beside the switch.
    pub label: String,
    /// One line explaining what turning it on does.
    pub hint: String,
    /// Whether it is on.
    pub checked: bool,
    /// Whether it can be used right now.
    pub available: bool,
    /// Why it cannot, when it cannot.
    pub reason: String,
}

impl ToggleViewModel {
    /// A switch that is on offer.
    pub fn new(id: &str, label: &str, hint: &str, checked: bool) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            hint: hint.to_string(),
            checked,
            available: true,
            reason: String::new(),
        }
    }

    /// The switch, unavailable with a reason the user can act on.
    pub fn blocked(mut self, reason: impl Into<String>) -> Self {
        self.available = false;
        self.reason = reason.into();
        self
    }
}

/// A number as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NumberViewModel {
    /// Element identifier of the control.
    pub id: String,
    /// Label above the control.
    pub label: String,
    /// What the user typed, kept exactly as typed.
    pub text: String,
    /// The typed value, when it reads as a number.
    pub value: Option<f64>,
    /// Unit shown after the value.
    pub unit: String,
    /// Smallest value the encoder accepts.
    pub min: Option<f64>,
    /// Largest value the encoder accepts.
    pub max: Option<f64>,
    /// Whether the control can be used right now.
    pub available: bool,
    /// Why it cannot, when it cannot.
    pub reason: String,
    /// Whether the typed value is usable.
    pub valid: bool,
    /// Why it is not, when it is not.
    pub problem: String,
}

impl NumberViewModel {
    /// A number control with no bounds of its own.
    pub fn new(id: &str, label: &str, text: &str, unit: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            text: text.to_string(),
            value: None,
            unit: unit.to_string(),
            min: None,
            max: None,
            available: true,
            reason: String::new(),
            valid: true,
            problem: String::new(),
        }
    }

    /// Bounds the encoder places on this value.
    pub fn ranged(mut self, min: f64, max: f64) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }

    /// The control, unavailable with a reason the user can act on.
    pub fn blocked(mut self, reason: impl Into<String>) -> Self {
        self.available = false;
        self.reason = reason.into();
        self
    }

    /// The control, marked unusable with a reason the user can act on.
    pub fn invalid(mut self, problem: impl Into<String>) -> Self {
        self.valid = false;
        self.problem = problem.into();
        self
    }
}

/// A line of text as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextViewModel {
    /// Element identifier of the control.
    pub id: String,
    /// Label above the control.
    pub label: String,
    /// What the user typed, kept exactly as typed.
    pub text: String,
    /// Text shown while the field is empty.
    pub placeholder: String,
    /// Whether the control can be used right now.
    pub available: bool,
    /// Why it cannot, when it cannot.
    pub reason: String,
    /// Whether what was typed is usable.
    pub valid: bool,
    /// Why it is not, when it is not.
    pub problem: String,
}

impl TextViewModel {
    /// A text field that is on offer.
    pub fn new(id: &str, label: &str, text: &str, placeholder: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            text: text.to_string(),
            placeholder: placeholder.to_string(),
            available: true,
            reason: String::new(),
            valid: true,
            problem: String::new(),
        }
    }

    /// The field, unavailable with a reason the user can act on.
    pub fn blocked(mut self, reason: impl Into<String>) -> Self {
        self.available = false;
        self.reason = reason.into();
        self
    }

    /// The field, marked unusable with a reason the user can act on.
    pub fn invalid(mut self, problem: impl Into<String>) -> Self {
        self.valid = false;
        self.problem = problem.into();
        self
    }
}

/// A number the user typed.
///
/// The text is kept as typed so the field never rewrites itself under the
/// cursor, and it is read back as a number only where a number is needed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct NumField {
    /// Exactly what the user typed.
    pub text: String,
}

impl NumField {
    /// A field holding `value`.
    pub fn of(value: impl ToString) -> Self {
        Self {
            text: value.to_string(),
        }
    }

    /// An empty field, which means "leave this to the encoder".
    pub fn blank() -> Self {
        Self::default()
    }

    /// Whether the field holds nothing.
    pub fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
    }

    /// Read the field as a whole number of `label`.
    ///
    /// An empty field is `Ok(None)`; anything that is not a number is an error
    /// naming what was typed and what was expected.
    pub fn whole(&self, label: &str) -> Result<Option<u32>, String> {
        let text = self.text.trim();
        if text.is_empty() {
            return Ok(None);
        }
        text.parse::<u32>()
            .map(Some)
            .map_err(|_| format!("{label} must be a whole number, not '{text}'."))
    }

    /// Read the field as a number of `label`.
    pub fn number(&self, label: &str) -> Result<Option<f64>, String> {
        let text = self.text.trim();
        if text.is_empty() {
            return Ok(None);
        }
        let value = text
            .parse::<f64>()
            .map_err(|_| format!("{label} must be a number, not '{text}'."))?;
        if !value.is_finite() {
            return Err(format!("{label} must be a finite number."));
        }
        Ok(Some(value))
    }
}

/// Read a number that has to sit inside a range.
pub fn number_in_range(
    value: Option<u32>,
    min: u32,
    max: u32,
    label: &str,
) -> Result<Option<u32>, String> {
    match value {
        None => Ok(None),
        Some(value) if value < min || value > max => Err(format!(
            "{label} must be between {min} and {max}, not {value}."
        )),
        Some(value) => Ok(Some(value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_group_keeps_the_options_it_was_given() {
        let group = ChoiceGroup::of([
            ("none", "None"),
            ("manual", "Manual"),
            ("aspect", "Aspect ratio"),
        ]);
        assert_eq!(group.options.len(), 3);
        assert!(group.options.iter().all(|option| option.available));
        assert_eq!(group.options[2].label, "Aspect ratio");
        let empty = ChoiceGroup::of(Vec::<(&str, &str)>::new());
        assert!(empty.options.is_empty());
    }

    #[test]
    fn blocked_options_keep_their_reason() {
        let options = vec![
            ChoiceOption::new("none", "None"),
            ChoiceOption::taken("manual", "Manual", "No frame size was reported."),
        ];
        let view = ChoiceGroupViewModel::build(
            "cropMode",
            "Crop mode",
            "How much of the frame is kept.",
            "none",
            &options,
            true,
            "",
        );
        assert!(!view.options[1].available);
        assert_eq!(view.options[1].reason, "No frame size was reported.");
        assert!(view.options[0].selected);
        assert!(!view.options[1].selected);
    }

    #[test]
    fn a_number_field_reads_whole_numbers_and_reports_anything_else() {
        assert_eq!(NumField::blank().whole("Width").expect("blank"), None);
        assert_eq!(
            NumField::of(1280).whole("Width").expect("number"),
            Some(1280)
        );
        assert_eq!(
            NumField::of(" 640 ").whole("Width").expect("spaces"),
            Some(640)
        );
        let error = NumField::of("12.5").whole("Width").expect_err("fraction");
        assert!(error.contains("Width"), "{error}");
        assert!(error.contains("12.5"), "{error}");
        assert!(NumField::of("wide")
            .whole("Width")
            .expect_err("text")
            .contains("whole number"));
        assert!(NumField::of("nan")
            .number("Rate")
            .expect_err("nan")
            .contains("finite"));
    }

    #[test]
    fn ranged_numbers_are_checked_against_their_bounds() {
        assert_eq!(
            number_in_range(Some(20), 0, 51, "Quality").expect("inside"),
            Some(20)
        );
        assert_eq!(
            number_in_range(None, 0, 51, "Quality").expect("unset"),
            None
        );
        let error = number_in_range(Some(80), 0, 51, "Quality").expect_err("high");
        assert!(error.contains("between 0 and 51"), "{error}");
    }
}
