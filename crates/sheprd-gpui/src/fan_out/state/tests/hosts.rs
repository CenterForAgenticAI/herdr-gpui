use super::*;
use crate::fan_out::journal::Resume;

#[test]
fn a_missing_host_can_rejoin_the_comparison_without_replaying_any_work() {
    let journal = Journal::default();
    let lanes = plan::lanes(&launched().picks, 7);
    let mut record = Record::new(42, &origin(), "go", &lanes);
    let mut remote = checkout(0);
    remote.endpoint_id = "saved-host".into();
    record.advance(&Report::Lane(
        0,
        Progress::Created(Box::new(remote.clone())),
    ));
    let _ = journal.begin(record.clone()).unwrap();
    let mut restored =
        FanOut::restore(record, journal, |id| (id == "local").then(|| origin().host)).unwrap();
    assert!(restored.lanes[0].checkout.is_none());
    let different = crate::teleport::host_for(&sheprd_client::ConnectTarget::Session {
        name: "different".into(),
        development: false,
    })
    .unwrap();
    assert!(!restored.reconnect(|_| Some(different.clone())));
    assert!(restored.reconnect(|_| Some(origin().host)));
    assert_eq!(restored.lanes[0].checkout, Some(remote));
    assert_eq!(restored.resumable(0), Some(Resume::Start));
    assert_eq!(restored.poll(), (false, None));
}
