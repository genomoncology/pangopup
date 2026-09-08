//! The published score value space.
//!
//! `spec/score-value.md` tells a consumer that a score is one of 101 exact
//! hundredths, that it always renders with two decimal places, and that a
//! threshold finer than one hundredth has no value to compare against. This
//! test holds those sentences to the shipped types.
//!
//! It lives outside `src/lib.rs` on purpose. The SNV builder's published source
//! fingerprint hashes that file whole, so a test added there would move the
//! builder's provenance identity.

use pangopup_core::{PangolinScore, RelativePosition, ScoreMagnitude, ValueError};

fn placeholder_position() -> RelativePosition {
    RelativePosition::new(0).expect("a relative position of zero")
}

fn rendered_loss(hundredths: u16) -> String {
    PangolinScore::new(
        ScoreMagnitude::new(0).expect("a zero gain"),
        placeholder_position(),
        ScoreMagnitude::new(hundredths).expect("hundredth in range"),
        placeholder_position(),
    )
    .loss_text()
    .to_string()
}

#[test]
fn a_score_value_is_one_of_one_hundred_and_one_exact_hundredths() {
    let gain: Vec<String> = (0..=100)
        .map(|hundredths| {
            ScoreMagnitude::new(hundredths)
                .expect("hundredth in range")
                .to_string()
        })
        .collect();
    assert_eq!(gain.len(), 101);
    assert_eq!(gain.first().map(String::as_str), Some("0.00"));
    assert_eq!(gain.last().map(String::as_str), Some("1.00"));
    let mut distinct = gain.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), 101, "every hundredth renders differently");
    for value in &gain {
        let (whole, fraction) = value.split_once('.').expect("a decimal point");
        assert!(whole == "0" || whole == "1", "{value}");
        assert_eq!(fraction.len(), 2, "{value} carries two decimal places");
    }
    assert_eq!(
        ScoreMagnitude::new(101),
        Err(ValueError::ScoreOutOfRange(101))
    );
}

#[test]
fn a_threshold_finer_than_one_hundredth_has_no_value_to_compare_against() {
    // 0.106 lies between two adjacent representable values. No response
    // distinguishes them, so a rule written against 0.106 is a rule against one
    // of the two.
    let below = ScoreMagnitude::new(10).expect("the neighbour below 0.106");
    let above = ScoreMagnitude::new(11).expect("the neighbour above 0.106");
    assert_eq!(below.to_string(), "0.10");
    assert_eq!(above.to_string(), "0.11");
    assert_eq!(above.hundredths() - below.hundredths(), 1, "adjacent");
    let representable: Vec<String> = (0..=100)
        .map(|hundredths| {
            ScoreMagnitude::new(hundredths)
                .expect("hundredth in range")
                .to_string()
        })
        .collect();
    assert!(
        !representable
            .iter()
            .any(|value| value.as_str() > "0.10" && value.as_str() < "0.11"),
        "nothing is representable between 0.10 and 0.11"
    );
}

#[test]
fn a_loss_carries_the_same_value_space_with_its_sign_restored() {
    let loss: Vec<String> = (0..=100).map(rendered_loss).collect();
    assert_eq!(loss.len(), 101);
    assert_eq!(loss.first().map(String::as_str), Some("0.00"));
    assert_eq!(loss.last().map(String::as_str), Some("-1.00"));
    assert!(
        !loss.iter().any(|value| value == "-0.00"),
        "a zero loss never renders a negative zero"
    );
    for value in loss.iter().skip(1) {
        let magnitude = value.strip_prefix('-').expect("a signed loss");
        assert_eq!(
            magnitude.split_once('.').expect("a decimal point").1.len(),
            2,
            "{value} carries two decimal places"
        );
    }
}
