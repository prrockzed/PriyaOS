//! What the plan says: features and their status (docs/claude/features.md)
//! and which sub-phases are finished (their **Done when** boxes, in
//! docs/claude/phases/phase-*.md).

use std::collections::BTreeMap;

use anyhow::{Result, bail};
use serde::Serialize;

use crate::tags::{is_feature_id, is_sub_phase};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Planned,
    InProgress,
    Done,
    Deferred,
}

impl Status {
    fn parse(cell: &str) -> Option<Self> {
        match cell {
            "⬜" => Some(Self::Planned),
            "🟨" => Some(Self::InProgress),
            "✅" => Some(Self::Done),
            "⏸" => Some(Self::Deferred),
            _ => None,
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Self::Planned => "⬜",
            Self::InProgress => "🟨",
            Self::Done => "✅",
            Self::Deferred => "⏸",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Feature {
    pub id: String,
    pub name: String,
    pub phase: String,
    pub status: Status,
}

/// Every row of the feature tables: `| ID | Feature | Src | Phase | Status |`.
pub fn features(markdown: &str) -> Result<BTreeMap<String, Feature>> {
    let mut features = BTreeMap::new();
    for (i, line) in markdown.lines().enumerate() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        // Only rows whose first cell is a feature ID are features.
        if cells.len() < 2 || !is_feature_id(cells[1]) {
            continue;
        }
        // A feature row splits into ["", ID, Feature, Src, Phase, Status, ""].
        let [_, id, name, _src, phase, status, _] = cells.as_slice() else {
            bail!(
                "features.md:{}: {} needs 5 columns: ID, Feature, Src, Phase, Status",
                i + 1,
                cells[1]
            );
        };
        let Some(status) = Status::parse(status) else {
            bail!("features.md:{}: {id} has unknown status `{status}`", i + 1);
        };
        let feature = Feature {
            id: (*id).to_owned(),
            name: (*name).to_owned(),
            phase: (*phase).to_owned(),
            status,
        };
        if features.insert(feature.id.clone(), feature).is_some() {
            bail!("features.md:{}: {id} appears twice", i + 1);
        }
    }
    Ok(features)
}

/// A sub-phase is finished when it has Done-when boxes and all are ticked.
pub fn finished_sub_phases(phase_file: &str) -> Vec<String> {
    let mut finished = Vec::new();
    let mut current: Option<(String, bool, usize)> = None; // id, all ticked, boxes
    let mut in_done_when = false;

    let mut close = |current: &mut Option<(String, bool, usize)>| {
        if let Some((id, all_ticked, boxes)) = current.take()
            && all_ticked
            && boxes > 0
        {
            finished.push(id);
        }
    };

    for line in phase_file.lines() {
        if line.starts_with("## ") || line.starts_with("### ") {
            close(&mut current);
            in_done_when = false;
            if let Some(id) = sub_phase_heading(line) {
                current = Some((id, true, 0));
            }
        } else if line.trim() == "**Done when**" {
            in_done_when = true;
        } else if line.starts_with("**") || line == "---" {
            in_done_when = false;
        } else if in_done_when && let Some((_, all_ticked, boxes)) = current.as_mut() {
            if line.starts_with("- [x]") || line.starts_with("- [X]") {
                *boxes += 1;
            } else if line.starts_with("- [ ]") {
                *boxes += 1;
                *all_ticked = false;
            }
        }
    }
    close(&mut current);
    finished
}

/// `### 0.4 — Test infrastructure` → `0.4`.
fn sub_phase_heading(line: &str) -> Option<String> {
    let rest = line.strip_prefix("### ")?;
    let id = rest.split_whitespace().next()?;
    is_sub_phase(id).then(|| id.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: QA-05
    // suite: unit
    #[test]
    fn features_table_rows_are_read_with_status() {
        let md = "## Core\n\n| ID | Feature | Src | Phase | Status |\n|---|---|---|---|---|\n| CORE-01 | Event log | C | 0 | 🟨 |\n| CORE-02 | Modules | C | 0 | ⬜ |\n";
        let features = features(md).unwrap();
        assert_eq!(features.len(), 2);
        assert_eq!(features["CORE-01"].status, Status::InProgress);
        assert_eq!(features["CORE-01"].phase, "0");
        assert!(super::features("| X-01 | a | C | 0 | ? |\n").is_err());
        assert!(
            super::features("| X-01 | a | 0 | ⬜ |\n").is_err(),
            "a missing column"
        );
    }

    // covers: QA-05
    // suite: unit
    #[test]
    fn a_sub_phase_is_finished_when_every_done_when_box_is_ticked() {
        let phase = "\
### 0.1 — First
**Tasks**
- [ ] not counted: a task
**Done when**
- [x] one
- [x] two

---

### 0.2 — Second
**Done when**
- [x] one
- [ ] two

### 0.3 — Third, no boxes yet
**Tasks**
- [x] a task
";
        assert_eq!(finished_sub_phases(phase), ["0.1"]);
    }
}
