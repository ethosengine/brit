use brit_epr::engine::cid::BritCid;
use brit_epr::engine::content_node::ContentNode;
use brit_epr::elohim::attestation::build::BuildAttestationContentNode;
use brit_epr::elohim::attestation::deploy::{DeployAttestationContentNode, HealthStatus};
use brit_epr::elohim::attestation::validation::{ValidationAttestationContentNode, ValidationResult};

fn sample_cid() -> BritCid { BritCid::compute(b"sample artifact") }

#[test]
fn build_attestation_roundtrips() {
    let node = BuildAttestationContentNode {
        manifest_cid: sample_cid(),
        step_name: "elohim-edge:cargo-build-storage".into(),
        inputs_hash: "abc123def456".into(),
        output_cid: BritCid::compute(b"output artifact"),
        agent_id: "deadbeef".repeat(4),
        hardware_profile: serde_json::json!({"arch": "x86_64", "os": "linux", "memory_gb": 32}),
        build_duration_ms: 45_000,
        built_at: "2026-04-16T10:00:00Z".into(),
        success: true,
        signature: "sig_placeholder".into(),
    };
    let json = serde_json::to_string_pretty(&node).unwrap();
    let back: BuildAttestationContentNode = serde_json::from_str(&json).unwrap();
    assert_eq!(node, back);
    assert_eq!(node.content_type(), "brit.build-attestation");
    let cid1 = node.compute_cid().unwrap();
    let cid2 = back.compute_cid().unwrap();
    assert_eq!(cid1, cid2);
}

#[test]
fn deploy_attestation_roundtrips() {
    let node = DeployAttestationContentNode {
        artifact_cid: sample_cid(),
        step_name: "elohim-edge:cargo-build-storage".into(),
        environment_label: "staging".into(),
        endpoint: "https://staging.elohim.host".into(),
        health_check_epr: "epr:staging-storage/health".into(),
        health_status: HealthStatus::Healthy,
        deployed_at: "2026-04-16T10:05:00Z".into(),
        attested_at: "2026-04-16T10:05:30Z".into(),
        liveness_ttl_sec: 300,
        agent_id: "deadbeef".repeat(4),
        signature: "sig_placeholder".into(),
    };
    let json = serde_json::to_string_pretty(&node).unwrap();
    let back: DeployAttestationContentNode = serde_json::from_str(&json).unwrap();
    assert_eq!(node, back);
    assert_eq!(node.content_type(), "brit.deploy-attestation");
}

#[test]
fn validation_attestation_roundtrips() {
    let node = ValidationAttestationContentNode {
        artifact_cid: sample_cid(),
        check_name: "sonarqube-scan@v10".into(),
        validator_id: "sonarqube-agent-001".into(),
        validator_version: "10.7.0".into(),
        result: ValidationResult::Pass,
        result_summary: "0 bugs, 0 vulnerabilities, 2 code smells".into(),
        findings_cid: None,
        validated_at: "2026-04-16T10:10:00Z".into(),
        ttl_sec: Some(86_400),
        signature: "sig_placeholder".into(),
    };
    let json = serde_json::to_string_pretty(&node).unwrap();
    let back: ValidationAttestationContentNode = serde_json::from_str(&json).unwrap();
    assert_eq!(node, back);
    assert_eq!(node.content_type(), "brit.validation-attestation");
}

#[test]
fn validation_result_serializes_as_lowercase() {
    assert_eq!(serde_json::to_string(&ValidationResult::Pass).unwrap(), r#""pass""#);
    assert_eq!(serde_json::to_string(&ValidationResult::Fail).unwrap(), r#""fail""#);
    assert_eq!(serde_json::to_string(&ValidationResult::Warn).unwrap(), r#""warn""#);
    assert_eq!(serde_json::to_string(&ValidationResult::Skip).unwrap(), r#""skip""#);
}

#[test]
fn health_status_serializes_as_lowercase() {
    assert_eq!(serde_json::to_string(&HealthStatus::Healthy).unwrap(), r#""healthy""#);
    assert_eq!(serde_json::to_string(&HealthStatus::Degraded).unwrap(), r#""degraded""#);
    assert_eq!(serde_json::to_string(&HealthStatus::Unreachable).unwrap(), r#""unreachable""#);
}
