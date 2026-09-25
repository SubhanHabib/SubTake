use super::*;
use std::time::{Duration, SystemTime};

#[test]
fn running_time_reads_as_a_clock() {
    assert_eq!(running_time(0.), "0:00");
    assert_eq!(running_time(84.4), "1:24");
    assert_eq!(running_time(59.6), "1:00");
    assert_eq!(running_time(3_723.), "1:02:03");
}

#[test]
fn relative_age_stays_short() {
    let ago = |seconds: u64| relative_age(SystemTime::now() - Duration::from_secs(seconds));
    assert_eq!(ago(5), "just now");
    assert_eq!(ago(2 * 60), "2m ago");
    assert_eq!(ago(3 * 3_600), "3h ago");
    assert_eq!(ago(4 * 86_400), "4d ago");
    assert_eq!(ago(9 * 86_400), "1w ago");
    assert_eq!(ago(90 * 86_400), "3mo ago");
}
