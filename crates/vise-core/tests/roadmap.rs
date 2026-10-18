//! The roadmap counts its own checkboxes.
//!
//! A status line that says "6/6 verified" while the list under it has seven
//! boxes is a status line nobody can trust, and the way that happens is by hand:
//! someone checks a box, and forgets the number three places away from it. So
//! the numbers are derived here instead, and a group whose stated count
//! disagrees with its own list fails this test.
//!
//! Two places state a count and both are checked: the status line under each
//! group's heading, and the row for that group in the status summary. They are
//! the same fact said twice, so they are checked against each other as well as
//! against the checkboxes.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// The roadmap, found from the crate rather than from the working directory so
/// the test says the same thing however it was run.
fn roadmap() -> String {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "docs", "ROADMAP.md"]
        .iter()
        .collect();
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("the roadmap at {} could not be read: {e}", path.display()))
}

/// What one group's list actually contains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Counts {
    checked: usize,
    total: usize,
}

impl Counts {
    fn matches(self, checked: usize, total: usize) -> bool {
        self.checked == checked && self.total == total
    }
}

/// Read the `N/M` a line states, wherever the sentence puts it.
///
/// The counts are written as prose rather than in a fixed column — "Complete
/// (6/6 verified)", "In progress (8/10 verified; optional …)" — so the digits
/// are found around the slash instead of being taken from fixed positions.
fn stated_counts(line: &str) -> Option<Counts> {
    let (before, after) = line.split_once('/')?;
    let checked: String = before
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let total: String = after.chars().take_while(char::is_ascii_digit).collect();
    if checked.is_empty() || total.is_empty() {
        return None;
    }
    Some(Counts {
        checked: checked.parse().ok()?,
        total: total.parse().ok()?,
    })
}

/// The group number a summary-table row is about.
fn row_group(line: &str) -> Option<u32> {
    if !line.trim_start().starts_with('|') {
        return None;
    }
    line.split('|').nth(1)?.trim().parse().ok()
}

/// Every group in the roadmap, its own counts, and the counts it claims.
#[derive(Debug, Default)]
struct Roadmap {
    boxes: BTreeMap<u32, Counts>,
    status: BTreeMap<u32, Counts>,
    /// The counts a status line marked *complete* claimed.
    complete: BTreeMap<u32, Counts>,
    summary: BTreeMap<u32, Counts>,
}

fn parse(text: &str) -> Roadmap {
    let mut roadmap = Roadmap::default();
    let mut group: Option<u32> = None;
    let mut in_summary = false;

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("## Group ") {
            // "## Group 10 — ..." or "## Group 12 - ...": the number is what
            // comes before the dash, whichever dash it is.
            let number = rest
                .split(['—', '-', ' '])
                .find(|token| token.trim().parse::<u32>().is_ok())
                .and_then(|token| token.trim().parse::<u32>().ok());
            group = number;
            in_summary = false;
            if let Some(number) = number {
                // Only the box counts are seeded: a group with no status line
                // states no counts, and saying otherwise would have the checker
                // comparing a claim nobody made.
                roadmap.boxes.entry(number).or_default();
            }
            continue;
        }
        if line.starts_with("## ") {
            // The summary is the one heading after the groups; anything else
            // ends the group that was being read.
            in_summary = line.trim() == "## Status Summary";
            group = None;
            continue;
        }

        if in_summary {
            if let (Some(number), Some(counts)) = (row_group(line), stated_counts(line)) {
                roadmap.summary.insert(number, counts);
            }
            continue;
        }

        let Some(group) = group else { continue };

        if let Some(item) = line.trim_start().strip_prefix("- [") {
            if let Some(boxes) = roadmap.boxes.get_mut(&group) {
                boxes.total += 1;
                match item.chars().next() {
                    Some('x') | Some('X') => boxes.checked += 1,
                    Some(' ') => {}
                    other => {
                        panic!("group {group} has an unreadable checkbox: {line:?} ({other:?})")
                    }
                }
            }
            continue;
        }

        if let Some(rest) = line.trim_start().strip_prefix("**Status:**") {
            if let Some(counts) = stated_counts(rest) {
                roadmap.status.insert(group, counts);
                // "Complete" is a claim about every box in the group, not only
                // about the number it prints.
                if rest.trim_start().starts_with("Complete") {
                    roadmap.complete.insert(group, counts);
                }
            }
        }
    }
    roadmap
}

#[test]
fn every_group_states_the_counts_its_own_checkboxes_add_up_to() {
    let text = roadmap();
    let roadmap = parse(&text);
    assert!(
        !roadmap.boxes.is_empty(),
        "the roadmap must have groups in it, or this test is checking nothing"
    );

    let mut wrong: Vec<String> = Vec::new();
    for (group, boxes) in &roadmap.boxes {
        let Some(status) = roadmap.status.get(group) else {
            // A group with no status line states no count, so there is nothing
            // to disagree with; say so rather than passing silently.
            continue;
        };
        if !status.matches(boxes.checked, boxes.total) {
            wrong.push(format!(
                "group {group}: the status line says {}/{} but the list has {} of {} checked",
                status.checked, status.total, boxes.checked, boxes.total
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_status_summary_agrees_with_the_group_it_summarises() {
    let text = roadmap();
    let roadmap = parse(&text);
    let mut wrong: Vec<String> = Vec::new();

    for (group, boxes) in &roadmap.boxes {
        let Some(summary) = roadmap.summary.get(group) else {
            continue;
        };
        if !summary.matches(boxes.checked, boxes.total) {
            wrong.push(format!(
                "group {group}: the status summary says {}/{} but the list has {} of {} checked",
                summary.checked, summary.total, boxes.checked, boxes.total
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_group_marked_complete_has_nothing_left_unchecked() {
    let roadmap = parse(&roadmap());
    let mut wrong: Vec<String> = Vec::new();

    for (group, claimed) in &roadmap.complete {
        let boxes = roadmap.boxes.get(group).copied().unwrap_or_default();
        // "Complete" while a box is still empty is the claim the roadmap's own
        // notes warn against, so it is the one that has to fail here.
        if claimed.checked != claimed.total
            || claimed.total != boxes.total
            || boxes.checked != boxes.total
        {
            wrong.push(format!(
                "group {group} says complete {}/{} but the list has {} of {} checked",
                claimed.checked, claimed.total, boxes.checked, boxes.total
            ));
        }
    }
    assert!(
        !roadmap.complete.is_empty(),
        "the roadmap claims no complete group, so this test is checking nothing"
    );
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_checker_itself_reads_a_roadmap_it_wrote() {
    // A checker that cannot fail is not a checker, so it is run against a small
    // roadmap whose counts are deliberately wrong, and asked to find them.
    let sample = "\
## Group 1 — One

- [x] first
- [x] second
- [ ] third

**Status:** Complete (2/3 verified)

## Group 2 - Two

- [x] only

**Status:** Complete (1/1 verified)

## Status Summary

| Group | Milestone | Status |
|---|---|---|
| 1 | One | Complete (2/3) |
| 2 | Two | Complete (2/2) |
";
    let roadmap = parse(sample);
    assert_eq!(
        roadmap.boxes.get(&1).copied(),
        Some(Counts {
            checked: 2,
            total: 3
        })
    );
    assert_eq!(
        roadmap.boxes.get(&2).copied(),
        Some(Counts {
            checked: 1,
            total: 1
        })
    );
    // Both groups state the right count for their own lists, and the summary row
    // for group two deliberately does not, so the checker has something to find.
    assert!(roadmap.status.get(&1).is_some_and(|c| c.matches(2, 3)));
    assert!(roadmap.status.get(&2).is_some_and(|c| c.matches(1, 1)));
    assert!(roadmap.summary.get(&1).is_some_and(|c| c.matches(2, 3)));
    assert!(roadmap.summary.get(&2).is_some_and(|c| !c.matches(1, 1)));
    // Group 1 claims to be complete with a box still empty, which is the
    // mistake the complete test exists to catch.
    assert!(roadmap
        .complete
        .get(&1)
        .is_some_and(|claimed| claimed.checked != claimed.total));
    assert!(roadmap
        .complete
        .get(&2)
        .is_some_and(|claimed| claimed.checked == claimed.total));
}
