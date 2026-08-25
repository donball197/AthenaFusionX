//! Integration-style unit tests: model X must resolve to endpoint Y.
//! Run with: cargo test -p nexus_runtime_core provider_routing -- --nocapture

use nexus_runtime_core::provider::{resolve_provider, AuthStyle};

#[test]
fn gemini_hits_google() {
    let r = resolve_provider("gemini-2.0-flash");
    assert!(
        r.endpoint.contains("generativelanguage.googleapis.com"),
        "Gemini must hit Google, got {}",
        r.endpoint
    );
    assert_eq!(r.provider_name, "google");
}

#[test]
fn nemotron_hits_nvidia() {
    let r = resolve_provider("nvidia/nemotron-3-ultra-550b-a55b");
    assert!(
        r.endpoint.contains("integrate.api.nvidia.com"),
        "Nemotron must hit NVIDIA NIM, got {}",
        r.endpoint
    );
    assert_eq!(r.provider_name, "nvidia");
    assert_eq!(r.auth, AuthStyle::Bearer);
}

#[test]
fn deepseek_hits_nvidia() {
    let r = resolve_provider("deepseek-ai/deepseek-v4-flash-0731");
    assert!(
        r.endpoint.contains("integrate.api.nvidia.com"),
        "DeepSeek must hit NVIDIA NIM, got {}",
        r.endpoint
    );
}

#[test]
fn plain_gemma_does_not_guess_google_or_nvidia() {
    let r = resolve_provider("gemma-2-9b-it");
    assert_eq!(
        r.provider_name, "nexus",
        "Bare model family names must use Nexus endpoint, not guess a cloud provider"
    );
}

#[test]
fn plain_llama_does_not_guess() {
    let r = resolve_provider("llama-3.3-70b");
    assert_eq!(r.provider_name, "nexus");
}
