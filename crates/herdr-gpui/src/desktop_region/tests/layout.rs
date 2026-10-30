//! The panel's width rules: a daemon's size can hide the panel but never
//! squeeze the panes below their minimum.
use super::*;

#[test]
fn a_size_is_held_to_the_regions_own_bounds() {
    let region = region(36, true, false);
    assert_eq!(columns(&region, 36, 200), Some(36));
    assert_eq!(columns(&region, 4, 200), Some(12), "raised to its minimum");
    assert_eq!(
        columns(&region, u16::MAX, 200),
        Some(60),
        "held to its maximum"
    );
    assert_eq!(columns(&region, 0, 200), None, "zero is hidden");
}

#[test]
fn a_size_that_would_squeeze_the_panes_hides_the_region() {
    let region = region(60, true, false);
    assert_eq!(
        columns(&region, 60, 80),
        Some(60),
        "leaves exactly the minimum"
    );
    assert_eq!(columns(&region, 60, 79), None);
    assert_eq!(
        columns(&region, 60, 10),
        None,
        "a row narrower than the region"
    );
}

#[test]
fn bounds_given_backwards_or_zero_still_bound() {
    let mut region = region(36, true, false);
    (region.min_size, region.max_size) = (60, 12);
    assert_eq!(columns(&region, 100, 200), Some(60));
    assert_eq!(columns(&region, 1, 200), Some(12));
    (region.min_size, region.max_size) = (0, 0);
    assert_eq!(columns(&region, 36, 200), Some(1));
}

#[test]
fn a_drag_stops_where_the_panes_keep_their_minimum() {
    let region = region(36, true, false);
    assert_eq!(dragged(&region, 40.4, 200), Some(40));
    assert_eq!(dragged(&region, 1000., 200), Some(60));
    assert_eq!(dragged(&region, 1000., 70), Some(50), "the panes keep 20");
    assert_eq!(dragged(&region, -5., 200), Some(12));
    assert_eq!(dragged(&region, 40., 25), None, "no width fits");
    assert_eq!(dragged(&region, f32::NAN, 200), None);
}
