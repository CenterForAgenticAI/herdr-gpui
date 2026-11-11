use super::*;

/// A clock well past every edit the tests make.
const NOW: u64 = 1_000_000;

fn shortcut(id: &str, title: &str, text: &str) -> TextShortcut {
    TextShortcut {
        id: id.into(),
        title: title.into(),
        text: text.into(),
    }
}

fn list(texts: &[&str], updated_ms: u64) -> TextShortcuts {
    TextShortcuts::new(
        texts
            .iter()
            .enumerate()
            .map(|(index, text)| shortcut(&format!("s{index}"), "", text)),
        updated_ms,
    )
}

#[test]
fn a_newer_copy_on_a_computer_is_taken() {
    let current = list(&["carry on"], 10);
    let stood = list(&["carry on", "explain"], 20);
    assert_eq!(
        settle(&current, 10, stood.clone(), NOW),
        Settled::Take(stood)
    );
}

#[test]
fn a_computer_that_took_the_offer_changes_nothing() {
    let current = list(&["carry on"], 10);
    assert_eq!(settle(&current, 10, current.clone(), NOW), Settled::Keep);
    // A daemon whose clock runs behind keeps the offer at its own time,
    // which the copy here takes, so it is not offered again as newer.
    assert_eq!(
        settle(&current, 10, list(&["carry on"], 8), NOW),
        Settled::Restamp(list(&["carry on"], 8))
    );
}

#[test]
fn an_edit_made_during_an_exchange_is_offered_again() {
    let current = list(&["explain"], 30);
    assert_eq!(
        settle(&current, 10, list(&["carry on"], 10), NOW),
        Settled::Again
    );
    // Unless the computer's copy is newer still, which wins whole.
    let later = list(&["review"], 40);
    assert_eq!(
        settle(&current, 10, later.clone(), NOW),
        Settled::Take(later)
    );
}

/// Three computers holding copies of different ages converge on the newest
/// once each has met the app: the newest is taken, then offered to the rest.
#[test]
fn computers_converge_on_the_latest_edit() {
    use sheprd_remote_control::control::edited::Edited;
    let mut computers = [
        list(&["old"], 10),
        list(&["newest"], 30),
        list(&["middle"], 20),
    ];
    let mut copy = TextShortcuts::default();
    // Two rounds: the first finds the newest, the second hands it on.
    for _ in 0..2 {
        for computer in &mut computers {
            let offered_ms = copy.updated_ms;
            *computer = computer.clone().merged(copy.clone(), 100);
            if let Settled::Take(stood) = settle(&copy, offered_ms, computer.clone(), NOW) {
                copy = stood;
            }
        }
    }
    assert_eq!(copy, list(&["newest"], 30));
    assert!(computers.iter().all(|computer| *computer == copy));
}

#[test]
fn an_edit_here_is_later_than_the_copy_it_replaces() {
    let current = list(&["carry on"], 500);
    let edit = edited(&current, vec![shortcut("a", "", "explain")], 100);
    assert_eq!(edit.updated_ms, 501);
    let edit = edited(&current, vec![shortcut("a", "", "explain")], 900);
    assert_eq!(edit.updated_ms, 900);
    // Bounded as every daemon bounds it.
    let blank = edited(&current, vec![shortcut("a", "", "  ")], 900);
    assert!(blank.shortcuts.is_empty());
}

#[test]
fn a_query_matches_the_label_or_the_text() {
    let named = shortcut("a", "Carry on", "Please continue where you left off");
    assert!(matches(&named, ""));
    assert!(matches(&named, "carry"));
    assert!(matches(&named, "LEFT OFF"));
    assert!(!matches(&named, "review"));
    let untitled = shortcut("b", "", "Review the diff");
    assert!(matches(&untitled, "review"));
}

#[test]
fn a_shortcut_joins_the_draft_a_space_apart() {
    assert_eq!(drafted("", "carry on"), "carry on");
    assert_eq!(drafted("look at", "carry on"), "look at carry on");
    assert_eq!(drafted("look at ", "carry on"), "look at carry on");
    assert_eq!(drafted("first\n", "carry on"), "first\ncarry on");
}

#[test]
fn new_ids_are_ones_every_daemon_keeps() {
    let id = new_id();
    let kept = TextShortcuts::new([shortcut(&id, "", "text")], 1);
    assert_eq!(kept.shortcuts.len(), 1);
    assert_ne!(new_id(), id);
}

#[test]
fn a_hosts_answer_is_bounded_or_nothing() {
    assert_eq!(sync::parse(b"").unwrap(), None);
    assert_eq!(sync::parse(b" \n").unwrap(), None);
    let answer = br#"{"shortcuts":[{"id":"a","title":"","text":"go"},{"id":"bad id","text":"x"}],"updated_ms":7}"#;
    let parsed = sync::parse(answer).unwrap().unwrap();
    assert_eq!(parsed, TextShortcuts::new([shortcut("a", "", "go")], 7));
    assert!(matches!(sync::parse(b"nope"), Err(Error::Answer(_))));
}

#[test]
fn the_host_script_runs_the_subcommand_without_installing() {
    assert!(sync::SCRIPT.contains("shortcuts --offer"));
    assert!(!sync::SCRIPT.contains("install"));
    assert!(sync::SCRIPT.contains("cat >/dev/null"));
}

#[test]
fn the_editor_saves_deletes_and_reorders_in_place() {
    let list = vec![
        shortcut("a", "", "one"),
        shortcut("b", "", "two"),
        shortcut("c", "", "three"),
    ];
    let ids = |list: &[TextShortcut]| list.iter().map(|own| own.id.clone()).collect::<Vec<_>>();
    let saved = with_saved(&list, shortcut("b", "Two", "two!"));
    assert_eq!(ids(&saved), ["a", "b", "c"]);
    assert_eq!(saved[1].title, "Two");
    assert_eq!(
        ids(&with_saved(&list, shortcut("d", "", "four"))),
        ["a", "b", "c", "d"]
    );
    assert_eq!(ids(&without(&list, "b")), ["a", "c"]);
    assert_eq!(ids(&without(&list, "z")), ["a", "b", "c"]);
    assert_eq!(ids(&moved(&list, "b", true)), ["b", "a", "c"]);
    assert_eq!(ids(&moved(&list, "b", false)), ["a", "c", "b"]);
    assert_eq!(ids(&moved(&list, "a", true)), ["a", "b", "c"]);
    assert_eq!(ids(&moved(&list, "c", false)), ["a", "b", "c"]);
    assert_eq!(ids(&moved(&list, "z", true)), ["a", "b", "c"]);
}

#[test]
fn an_answer_edited_far_in_the_future_is_not_taken() {
    let current = list(&["carry on"], 10);
    let ahead = list(&["from a fast clock"], NOW + FUTURE_TOLERANCE_MS + 1);
    assert_eq!(settle(&current, 10, ahead, NOW), Settled::Keep);
    let slightly = list(&["a minute ahead"], NOW + 60_000);
    assert_eq!(
        settle(&current, 10, slightly.clone(), NOW),
        Settled::Take(slightly)
    );
    assert_eq!(
        settle(&current, 10, list(&["max"], u64::MAX), NOW),
        Settled::Keep
    );
}

#[test]
fn an_edit_after_the_largest_stamp_does_not_overflow() {
    let current = list(&["carry on"], u64::MAX);
    assert_eq!(edited(&current, Vec::new(), NOW).updated_ms, u64::MAX);
}

#[test]
fn a_copy_a_slower_clock_took_is_not_offered_again_as_newer() {
    // This clock runs ahead: the host took the offer stamped by its own.
    let current = list(&["carry on"], 5_000);
    let taken = list(&["carry on"], 4_000);
    assert_eq!(
        settle(&current, 5_000, taken.clone(), NOW),
        Settled::Restamp(taken.clone())
    );
    // A phone edit there, stamped by the host's clock after it, now wins
    // the next exchange instead of being replaced by the older copy.
    let phone = list(&["review the diff"], 4_500);
    assert_eq!(
        settle(&taken, 4_000, phone.clone(), NOW),
        Settled::Take(phone)
    );
}
