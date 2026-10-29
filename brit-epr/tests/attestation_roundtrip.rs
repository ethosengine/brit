use brit_epr::{
    elohim::attestation::{
        build::BuildAttestationContentNode,
        deploy::{DeployAttestationContentNode, HealthStatus},
        validation::{ValidationAttestationContentNode, ValidationResult},
    },
    engine::{
        cid::BritCid,
        content_node::ContentNode,
        signing::{verify_signed_node, AgentKey, Signed},
    },
};
use tempfile::TempDir;

fn sample_cid() -> BritCid {
    BritCid::compute(b"sample artifact")
}

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
    assert_eq!(
        serde_json::to_string(&HealthStatus::Unreachable).unwrap(),
        r#""unreachable""#
    );
}

#[test]
fn build_attestation_sign_store_retrieve_verify() {
    let tmp = TempDir::new().unwrap();
    let key = AgentKey::generate(&tmp.path().join("agent-key")).unwrap();
    let store = brit_epr::engine::object_store::LocalObjectStore::new(tmp.path().join("objects"));

    // Construct with empty signature
    let mut node = BuildAttestationContentNode {
        manifest_cid: sample_cid(),
        step_name: "test:step".into(),
        inputs_hash: "inputs-abc".into(),
        output_cid: BritCid::compute(b"output"),
        agent_id: key.agent_id(),
        hardware_profile: serde_json::json!({}),
        build_duration_ms: 1000,
        built_at: "2026-04-16T12:00:00Z".into(),
        success: true,
        signature: String::new(),
    };

    // Sign
    let canonical = node.canonical_bytes().unwrap();
    node.signature = key.sign(&canonical);

    // Store
    let cid = store.put(&node).unwrap();

    // Retrieve
    let back: BuildAttestationContentNode = store.get(&cid).unwrap();

    // Verify — must succeed
    assert!(verify_signed_node(&back).unwrap(), "signature verification should pass");

    // Tamper — must fail
    let mut tampered = back;
    tampered.success = false;
    assert!(
        !verify_signed_node(&tampered).unwrap(),
        "tampered node should fail verification"
    );
}

/// The note path: a build node is signed over its DAG-CBOR bytes, stored as JSON in a git note,
/// and read back from that JSON. Verification is only sound if the JSON form reproduces the signed
/// bytes exactly, for every value a `hardwareProfile` can carry and in any property order.
#[test]
fn json_note_round_trip_reproduces_the_signed_bytes() {
    let tmp = TempDir::new().unwrap();
    let key = AgentKey::generate(&tmp.path().join("agent-key")).unwrap();
    let profiles = [
        serde_json::json!({}),
        serde_json::json!({"z": {"y": {"x": [3, 2, 1]}}, "a": null, "m": {"b": null, "a": []}}),
        serde_json::json!({
            "u64max": u64::MAX, "i64min": i64::MIN, "zero": 0, "neg": -1,
            "past53": 9_007_199_254_740_993_u64, "i64max": i64::MAX
        }),
        serde_json::json!({
            "tenth": 0.1, "third": 0.30000000000000004, "tiny": 5e-324, "subnormal": 2.225_073_858_507_201e-308,
            "min_normal": 2.2250738585072014e-308, "huge": 1.797_693_134_862_315_7e308, "neg": -1.5e-300,
            "frac": 123_456_789.123_456_78, "one": 1.0
        }),
        serde_json::json!([null, true, "s", 1, -1, 0.5, {"k": [{}]}]),
        serde_json::Value::Null,
    ];
    for profile in profiles {
        let mut node = BuildAttestationContentNode {
            manifest_cid: sample_cid(),
            step_name: "gate:elohim-storage".into(),
            inputs_hash: "closure".into(),
            output_cid: BritCid::compute(b"output"),
            agent_id: key.agent_id(),
            hardware_profile: profile.clone(),
            build_duration_ms: u64::MAX,
            built_at: "2026-10-29T16:00:57.532831468+00:00".into(),
            success: false,
            signature: String::new(),
        };
        let signed_bytes = node.canonical_bytes().unwrap();
        node.signature = key.sign(&signed_bytes);

        // The writer's own path: typed node -> JSON value -> note bytes.
        let note = serde_json::to_vec(&serde_json::to_value(&node).unwrap()).unwrap();
        let back: BuildAttestationContentNode = serde_json::from_slice(&note).unwrap();
        assert_eq!(
            back.without_signature().canonical_bytes().unwrap(),
            signed_bytes,
            "profile {profile}"
        );
        assert!(verify_signed_node(&back).unwrap(), "profile {profile}");
        assert_eq!(back, node);

        // The same note with every top-level property in reverse order.
        let serde_json::Value::Object(map) = serde_json::to_value(&node).unwrap() else {
            unreachable!("a struct serializes to an object")
        };
        let reversed = format!(
            "{{{}}}",
            map.iter()
                .rev()
                .map(|(k, v)| format!("{}:{}", serde_json::to_string(k).unwrap(), v))
                .collect::<Vec<_>>()
                .join(",")
        );
        let back: BuildAttestationContentNode = serde_json::from_str(&reversed).unwrap();
        assert_eq!(back.without_signature().canonical_bytes().unwrap(), signed_bytes);
        assert!(verify_signed_node(&back).unwrap());
    }
}

#[test]
fn attestation_nodes_refuse_unknown_fields() {
    let mut build = serde_json::to_value(BuildAttestationContentNode {
        manifest_cid: sample_cid(),
        step_name: "s".into(),
        inputs_hash: "i".into(),
        output_cid: sample_cid(),
        agent_id: "a".into(),
        hardware_profile: serde_json::json!({}),
        build_duration_ms: 0,
        built_at: "t".into(),
        success: true,
        signature: String::new(),
    })
    .unwrap();
    build["trusted"] = serde_json::Value::Bool(true);
    assert!(serde_json::from_value::<BuildAttestationContentNode>(build).is_err());

    let mut deploy = serde_json::to_value(DeployAttestationContentNode {
        artifact_cid: sample_cid(),
        step_name: "s".into(),
        environment_label: "e".into(),
        endpoint: "x".into(),
        health_check_epr: "epr:x".into(),
        health_status: HealthStatus::Healthy,
        deployed_at: "t".into(),
        attested_at: "t".into(),
        liveness_ttl_sec: 1,
        agent_id: "a".into(),
        signature: String::new(),
    })
    .unwrap();
    deploy["admitted"] = serde_json::Value::Bool(true);
    assert!(serde_json::from_value::<DeployAttestationContentNode>(deploy).is_err());

    let mut validation = serde_json::to_value(ValidationAttestationContentNode {
        artifact_cid: sample_cid(),
        check_name: "c".into(),
        validator_id: "a".into(),
        validator_version: "v".into(),
        result: ValidationResult::Pass,
        result_summary: String::new(),
        findings_cid: None,
        validated_at: "t".into(),
        ttl_sec: None,
        signature: String::new(),
    })
    .unwrap();
    validation["standing"] = serde_json::Value::Bool(true);
    assert!(serde_json::from_value::<ValidationAttestationContentNode>(validation).is_err());
}
