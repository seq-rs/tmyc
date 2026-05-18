//! Smoke tests against real-world-shaped YAML fixtures.
//! Asserts shape only, not full content equality.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;

const FIXTURES: &str = "tests/fixtures";

#[derive(Deserialize, Debug)]
struct K8sPod {
    #[serde(rename = "apiVersion")]
    api_version: String,
    kind: String,
    metadata: Metadata,
    spec: PodSpec,
}

#[derive(Deserialize, Debug)]
struct Metadata {
    name: String,
    #[serde(default)]
    labels: BTreeMap<String, String>,
}

#[derive(Deserialize, Debug)]
struct PodSpec {
    containers: Vec<Container>,
    #[serde(rename = "restartPolicy", default)]
    restart_policy: Option<String>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct Container {
    name: String,
    image: String,
}

#[test]
fn k8s_pod_deserializes() {
    let src = fs::read_to_string(format!("{FIXTURES}/k8s_pod.yaml")).unwrap();
    let pod: K8sPod = tmyc::from_str(&src).unwrap();
    assert_eq!(pod.kind, "Pod");
    assert_eq!(pod.api_version, "v1");
    assert_eq!(pod.metadata.name, "web");
    assert_eq!(pod.metadata.labels.get("app").unwrap(), "nginx");
    assert_eq!(pod.spec.containers.len(), 1);
    assert_eq!(pod.spec.containers[0].image, "nginx:1.25");
    assert_eq!(pod.spec.restart_policy.as_deref(), Some("Always"));
}

#[test]
fn compose_merge_keys_applied() {
    let src = fs::read_to_string(format!("{FIXTURES}/compose.yaml")).unwrap();
    let value = tmyc::Parser::new(&src).parse().unwrap();
    // services.web should have `restart: always` from defaults
    let services = match &value {
        tmyc::Value::Map(pairs) => pairs
            .iter()
            .find_map(|(k, v)| match k {
                tmyc::Value::String(s) if s == "services" => Some(v),
                _ => None,
            })
            .expect("services key"),
        _ => panic!("expected top-level map"),
    };
    let web = match services {
        tmyc::Value::Map(pairs) => pairs
            .iter()
            .find_map(|(k, v)| match k {
                tmyc::Value::String(s) if s == "web" => Some(v),
                _ => None,
            })
            .expect("web key"),
        _ => panic!("expected services map"),
    };
    let web_pairs = match web {
        tmyc::Value::Map(p) => p,
        _ => panic!(),
    };
    let restart = web_pairs.iter().find_map(|(k, v)| match k {
        tmyc::Value::String(s) if s == "restart" => Some(v),
        _ => None,
    });
    assert!(
        matches!(restart, Some(tmyc::Value::String(s)) if s == "always"),
        "merge key didn't splice `restart: always` into web"
    );
    // api should have overridden restart to on-failure
    let api = match services {
        tmyc::Value::Map(pairs) => pairs
            .iter()
            .find_map(|(k, v)| match k {
                tmyc::Value::String(s) if s == "api" => Some(v),
                _ => None,
            })
            .expect("api key"),
        _ => panic!(),
    };
    let api_pairs = match api {
        tmyc::Value::Map(p) => p,
        _ => panic!(),
    };
    let api_restart = api_pairs.iter().find_map(|(k, v)| match k {
        tmyc::Value::String(s) if s == "restart" => Some(v),
        _ => None,
    });
    assert!(
        matches!(api_restart, Some(tmyc::Value::String(s)) if s == "on-failure"),
        "explicit api.restart should override merged-in value"
    );
}

#[test]
fn kubectl_stream_parses_multi_doc() {
    let src = fs::read_to_string(format!("{FIXTURES}/kubectl_stream.yaml")).unwrap();
    let docs = tmyc::Parser::new(&src).parse_all().unwrap();
    assert_eq!(docs.len(), 2);
}

#[test]
fn sops_secret_block_scalars_parse() {
    let src = fs::read_to_string(format!("{FIXTURES}/sops_secret.yaml")).unwrap();
    let value = tmyc::Parser::new(&src).parse().unwrap();
    let data = match &value {
        tmyc::Value::Map(pairs) => pairs
            .iter()
            .find_map(|(k, v)| match k {
                tmyc::Value::String(s) if s == "data" => Some(v),
                _ => None,
            })
            .expect("data key"),
        _ => panic!(),
    };
    let cert = match data {
        tmyc::Value::Map(pairs) => pairs
            .iter()
            .find_map(|(k, v)| match k {
                tmyc::Value::String(s) if s == "cert" => Some(v),
                _ => None,
            })
            .expect("cert key"),
        _ => panic!(),
    };
    let cert_str = match cert {
        tmyc::Value::String(s) => s.as_ref(),
        _ => panic!("expected string cert"),
    };
    // Literal block: newlines preserved between lines
    assert!(cert_str.starts_with("-----BEGIN CERTIFICATE-----"));
    assert!(cert_str.contains("\nMIID"));
    assert!(cert_str.ends_with("-----END CERTIFICATE-----\n"));

    // Folded description: newlines folded to spaces
    let description = match data {
        tmyc::Value::Map(pairs) => pairs
            .iter()
            .find_map(|(k, v)| match k {
                tmyc::Value::String(s) if s == "description" => Some(v),
                _ => None,
            })
            .expect("description key"),
        _ => panic!(),
    };
    let desc_str = match description {
        tmyc::Value::String(s) => s.as_ref(),
        _ => panic!(),
    };
    assert!(desc_str.contains("spans multiple lines but joins them"));
    // No internal newlines except possibly trailing
    let interior = &desc_str[..desc_str.len().saturating_sub(1)];
    assert!(!interior.contains('\n'), "folded scalar should join lines with spaces");
}
