use super::*;

#[test]
fn tabs_keep_their_herdr_unless_herdr_2_is_chosen_and_there() {
    use Generation::{Herdr1, Herdr2};
    // Herdr 1 → Herdr 1, unchanged, and Herdr 2 → Herdr 2.
    assert_eq!(destination_generation(Herdr1, true, false), Herdr1);
    assert_eq!(destination_generation(Herdr2, true, false), Herdr2);
    // Herdr 1 → Herdr 2, with Herdr 2 chosen for new tabs.
    assert_eq!(destination_generation(Herdr1, true, true), Herdr2);
    // Herdr 2 → Herdr 1, where the destination has no herdr-core.
    assert_eq!(destination_generation(Herdr2, false, false), Herdr1);
    assert_eq!(destination_generation(Herdr1, false, true), Herdr1);
}

#[test]
fn a_tab_too_big_for_herdr_2_stays_in_herdr_1() {
    let ids: Vec<String> = (0..=MAX_PANES).map(|n| format!("p{n}")).collect();
    let moves = (Generation::Herdr1, Generation::Herdr2);
    let big = plan_tree(moves, None, Node::row(&ids).unwrap(), &[], &[], &mut 0);
    assert_eq!(big.into, Generation::Herdr1);
    let small = plan_tree(moves, None, Node::row(&ids[..2]).unwrap(), &[], &[], &mut 0);
    assert_eq!(small.into, Generation::Herdr2);
    assert_eq!(small.generation, Generation::Herdr1);
}
