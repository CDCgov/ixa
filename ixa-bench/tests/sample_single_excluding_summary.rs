#[path = "../criterion/sample_single_excluding_summary.rs"]
mod summary;

use summary::{complete_rows, Measurements, SummaryRow};

#[test]
fn no_rows_are_returned_without_measurements() {
    assert!(complete_rows(&Measurements::new(), &[2, 3, 4]).is_empty());
}

#[test]
fn no_rows_are_returned_for_a_single_strategy() {
    let results = Measurements::from([
        (("rejection", 2), 2.1),
        (("rejection", 3), 3.1),
        (("rejection", 4), 4.1),
    ]);

    assert!(complete_rows(&results, &[2, 3, 4]).is_empty());
}

#[test]
fn only_complete_rows_are_returned() {
    let results = Measurements::from([
        (("rejection", 2), 2.1),
        (("iteration", 2), 2.2),
        (("rejection", 3), 3.1),
        (("iteration", 3), 3.2),
        (("auto", 3), 3.3),
    ]);

    assert_eq!(
        complete_rows(&results, &[2, 3, 4]),
        vec![SummaryRow {
            size: 3,
            rejection: 3.1,
            iteration: 3.2,
            automatic: 3.3,
        }]
    );
}
