use super::*;

#[core::prelude::v1::test]
fn with_nothing_running_the_plan_and_files_stay_open() {
    let folds = RailFolds::default();
    assert!(folds.is_open(RailSection::Plan, false));
    assert!(folds.is_open(RailSection::Changed, false));
}

#[core::prelude::v1::test]
fn running_work_opens_and_folds_the_plan_and_files_to_a_line() {
    let folds = RailFolds::default();
    assert!(folds.is_open(RailSection::Subagents, true));
    assert!(folds.is_open(RailSection::Shells, true));
    assert!(!folds.is_open(RailSection::Plan, true));
    assert!(!folds.is_open(RailSection::Changed, true));
}

#[core::prelude::v1::test]
fn a_click_outlasts_the_work_changing() {
    let now = Instant::now();
    let mut folds = RailFolds::default();
    folds.toggle(RailSection::Plan, true, now);
    assert!(folds.is_open(RailSection::Plan, true), "opened while busy");
    assert!(folds.is_open(RailSection::Plan, false));
    folds.toggle(RailSection::Plan, false, now);
    assert!(
        !folds.is_open(RailSection::Plan, false),
        "closed while idle"
    );
}

#[core::prelude::v1::test]
fn a_click_moves_from_where_the_section_stood() {
    let now = Instant::now();
    let mut folds = RailFolds::default();
    folds.toggle(RailSection::Shells, true, now);
    assert!(folds.moving(now));
    assert!(
        folds.shown(RailSection::Shells, true, now) > 0.9,
        "closing from open"
    );
}

#[core::prelude::v1::test]
fn what_ended_starts_folded_away_for_each_section_alone() {
    let now = Instant::now();
    let mut folds = RailFolds::default();
    assert!(!folds.ended_is_open(RailSection::Subagents));
    folds.toggle_ended(RailSection::Subagents, now);
    assert!(folds.ended_is_open(RailSection::Subagents));
    assert!(!folds.ended_is_open(RailSection::Shells));
}
