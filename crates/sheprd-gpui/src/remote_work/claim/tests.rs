use super::*;

const KEY: &str = "sheprd-work-0123456789abcdef0123456789abcdef";
const CLAIM: &str = "sheprd-work-claim-0123456789abcdef0123456789abcdef";

fn labels(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

#[test]
fn only_an_unclaimed_request_as_read_is_open() {
    assert!(open(&labels(&[(KEY, "req")]), KEY, CLAIM, "req"));
    assert!(!open(&labels(&[]), KEY, CLAIM, "req"));
    assert!(!open(&labels(&[(KEY, "other")]), KEY, CLAIM, "req"));
    assert!(!open(
        &labels(&[(KEY, "req"), (CLAIM, CLAIMED)]),
        KEY,
        CLAIM,
        "req"
    ));
}

#[test]
fn a_request_withdrawn_before_the_marker_is_not_held() {
    // The caller cancelled between the read and the marker: the core's
    // answer to the marker lacks the request, so the claim is abandoned.
    assert!(!held(&labels(&[(CLAIM, CLAIMED)]), KEY, CLAIM, "req"));
    assert!(held(
        &labels(&[(KEY, "req"), (CLAIM, CLAIMED)]),
        KEY,
        CLAIM,
        "req"
    ));
    assert!(!held(
        &labels(&[(KEY, "other"), (CLAIM, CLAIMED)]),
        KEY,
        CLAIM,
        "req"
    ));
}
