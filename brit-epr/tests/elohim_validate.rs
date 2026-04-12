//! Integration tests for elohim pillar structural validation.

use brit_epr::{validate_pillar_trailers, PillarTrailers, PillarValidationError, TrailerKey};

fn complete() -> PillarTrailers {
    PillarTrailers {
        lamad: Some("knowledge summary".into()),
        shefa: Some("economic summary".into()),
        qahal: Some("governance summary".into()),
        lamad_node: None,
        shefa_node: None,
        qahal_node: None,
    }
}

#[test]
fn all_three_present_validates_ok() {
    assert_eq!(validate_pillar_trailers(&complete()), Ok(()));
}

#[test]
fn missing_lamad_fails_with_missing_pillar() {
    let mut t = complete();
    t.lamad = None;
    assert_eq!(
        validate_pillar_trailers(&t),
        Err(PillarValidationError::MissingPillar(TrailerKey::Lamad))
    );
}

#[test]
fn empty_shefa_fails_with_empty_pillar() {
    let mut t = complete();
    t.shefa = Some("   ".into());
    assert_eq!(
        validate_pillar_trailers(&t),
        Err(PillarValidationError::EmptyPillar(TrailerKey::Shefa))
    );
}

#[test]
fn returns_first_error_in_canonical_order() {
    let t = PillarTrailers {
        lamad: None,
        shefa: Some("ok".into()),
        qahal: None,
        ..Default::default()
    };
    assert_eq!(
        validate_pillar_trailers(&t),
        Err(PillarValidationError::MissingPillar(TrailerKey::Lamad))
    );
}
