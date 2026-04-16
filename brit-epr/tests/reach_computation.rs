use brit_epr::elohim::attestation::reach::{compute_reach, ReachInput, ReachLevel};

#[test]
fn no_attestations_returns_unknown() {
    let input = ReachInput { build_attestations: vec![], deploy_attestations: vec![], validation_attestations: vec![] };
    assert_eq!(compute_reach(&input), ReachLevel::Unknown);
}

#[test]
fn build_only_returns_built() {
    let input = ReachInput { build_attestations: vec!["agent-a".into()], deploy_attestations: vec![], validation_attestations: vec![] };
    assert_eq!(compute_reach(&input), ReachLevel::Built);
}

#[test]
fn build_plus_deploy_returns_deployed() {
    let input = ReachInput { build_attestations: vec!["agent-a".into()], deploy_attestations: vec!["staging".into()], validation_attestations: vec![] };
    assert_eq!(compute_reach(&input), ReachLevel::Deployed);
}

#[test]
fn build_plus_deploy_plus_validation_returns_verified() {
    let input = ReachInput { build_attestations: vec!["agent-a".into()], deploy_attestations: vec!["staging".into()], validation_attestations: vec!["sonarqube-scan@v10".into()] };
    assert_eq!(compute_reach(&input), ReachLevel::Verified);
}

#[test]
fn same_inputs_same_result() {
    let input = ReachInput { build_attestations: vec!["agent-a".into(), "agent-b".into()], deploy_attestations: vec!["staging".into()], validation_attestations: vec!["trivy@latest".into(), "sonarqube@v10".into()] };
    let r1 = compute_reach(&input);
    let r2 = compute_reach(&input);
    assert_eq!(r1, r2, "reach computation must be deterministic");
}
