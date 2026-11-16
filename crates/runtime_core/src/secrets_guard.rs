#![allow(dead_code)]
//! Secret-material path guard and private-host check for ScrapeUrl.
//!
//! SCOPE AND LIMITS (read before extending):
//!
//! `is_secret_path` is a basename matcher. It catches `.env`, `.env.local`,
//! `*.pem`, `id_rsa`, and `.git/config` regardless of directory. It ALLOWS
//! `.env.example`, `.env.test`, `.env.sample`, and similar template
//! fixtures -- those contain placeholder values, not secrets, and the test
//! suite legitimately reads them.
//!
//! `host_is_private` is a STRING-ONLY check. It does NOT resolve DNS and
//! does NOT follow HTTP redirects. Two classes of bypass it does not catch:
//!
//!   1. A public hostname whose A record is a private IP (DNS rebinding).
//!   2. A public URL that returns 302 to a private address.
//!
//! A complete SSRF defense requires network-namespace sandboxing or a
//! redirect-aware HTTP client that re-validates each hop. This module is
//! the naive-case block, not the complete defense.

pub fn is_secret_path(path: &str) -> bool {
    let path_norm = path.replace('\\', "/");
    let base = path_norm.rsplit('/').next().unwrap_or(&path_norm).trim();
    if base.is_empty() {
        return false;
    }
    if base == ".env" {
        return true;
    }
    if let Some(rest) = base.strip_prefix(".env.") {
        const TEMPLATE_KINDS: &[&str] = &[
            "example", "sample", "template", "dist", "defaults", "test", "tests", "ci",
        ];
        let is_template = TEMPLATE_KINDS
            .iter()
            .any(|k| rest == *k || rest.starts_with(&format!("{}.", k)));
        if !is_template {
            return true;
        }
    }
    if base.ends_with(".pem")
        || base.ends_with(".key")
        || base.ends_with(".p12")
        || base.ends_with(".pfx")
    {
        return true;
    }
    if base == "id_rsa"
        || base == "id_ed25519"
        || base == "id_ecdsa"
        || base == "id_dsa"
        || base.starts_with("id_rsa.")
        || base.starts_with("id_ed25519.")
        || base.starts_with("id_ecdsa.")
    {
        return true;
    }
    if path_norm == ".git/config"
        || path_norm.ends_with("/.git/config")
        || path_norm == ".git/credentials"
        || path_norm.ends_with("/.git/credentials")
    {
        return true;
    }
    false
}

pub fn host_is_private(host: &str) -> bool {
    let h = host.trim().to_lowercase();
    if h.is_empty() {
        return true;
    }
    let bare = if h.starts_with('[') {
        h.trim_start_matches('[').split(']').next().unwrap_or("").to_string()
    } else {
        h.split(':').next().unwrap_or(&h).to_string()
    };
    if bare == "localhost" {
        return true;
    }
    if bare == "::1"
        || bare == "::"
        || bare.starts_with("fe80:")
        || bare.starts_with("fc")
        || bare.starts_with("fd")
    {
        return true;
    }
    let parts: Vec<&str> = bare.split('.').collect();
    if parts.len() == 4 {
        if let (Ok(a), Ok(b), _, _) = (
            parts[0].parse::<u8>(),
            parts[1].parse::<u8>(),
            parts[2].parse::<u8>(),
            parts[3].parse::<u8>(),
        ) {
            return a == 0
                || a == 10
                || a == 127
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && b == 168);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{host_is_private, is_secret_path};

    #[test]
    fn plain_dot_env_is_a_secret() {
        assert!(is_secret_path(".env"));
        assert!(is_secret_path("src/.env"));
        assert!(is_secret_path("/root/AthenaFusionX/.env"));
        assert!(is_secret_path(".env.local"));
        assert!(is_secret_path(".env.production"));
        assert!(is_secret_path(".git\\config"));
    }

    #[test]
    fn templates_and_fixtures_are_not_secrets() {
        assert!(!is_secret_path(".env.example"));
        assert!(!is_secret_path(".env.sample"));
        assert!(!is_secret_path(".env.template"));
        assert!(!is_secret_path(".env.test"));
        assert!(!is_secret_path(".env.dist"));
        assert!(!is_secret_path("src/.env.example"));
    }

    #[test]
    fn private_keys_are_secrets() {
        assert!(is_secret_path("key.pem"));
        assert!(is_secret_path("private.key"));
        assert!(is_secret_path("cert.p12"));
        assert!(is_secret_path("id_rsa"));
        assert!(is_secret_path("id_ed25519"));
        assert!(is_secret_path("~/.ssh/id_rsa.old"));
    }

    #[test]
    fn git_credentials_are_secrets() {
        assert!(is_secret_path(".git/config"));
        assert!(is_secret_path("subdir/.git/config"));
        assert!(is_secret_path(".git/credentials"));
    }

    #[test]
    fn ordinary_files_are_not_secrets() {
        assert!(!is_secret_path("README.md"));
        assert!(!is_secret_path("src/lib.rs"));
        assert!(!is_secret_path("config.yml"));
        assert!(!is_secret_path(""));
    }

    #[test]
    fn private_ipv4_addresses_are_blocked() {
        assert!(host_is_private("127.0.0.1"));
        assert!(host_is_private("127.0.0.1:8080"));
        assert!(host_is_private("10.0.0.1"));
        assert!(host_is_private("172.16.0.1"));
        assert!(host_is_private("172.31.255.255"));
        assert!(host_is_private("192.168.1.1"));
        assert!(host_is_private("169.254.169.254"));
        assert!(host_is_private("0.0.0.0"));
    }

    #[test]
    fn public_ipv4_addresses_are_allowed() {
        assert!(!host_is_private("8.8.8.8"));
        assert!(!host_is_private("1.1.1.1"));
        assert!(!host_is_private("172.32.0.1"));
        assert!(!host_is_private("172.15.255.255"));
    }

    #[test]
    fn ipv6_and_localhost() {
        assert!(host_is_private("localhost"));
        assert!(host_is_private("localhost:3000"));
        assert!(host_is_private("[::1]"));
        assert!(host_is_private("[::1]:8080"));
        assert!(host_is_private("[fe80::1]"));
        assert!(host_is_private("[fc00::1]"));
        assert!(!host_is_private("[2001:db8::1]"));
    }

    #[test]
    fn empty_host_fails_closed() {
        assert!(host_is_private(""));
        assert!(host_is_private("   "));
    }

    #[test]
    fn hostname_and_rebinding_not_caught_by_design() {
        assert!(!host_is_private("example.com"));
        assert!(!host_is_private("api.github.com"));
        assert!(!host_is_private("internal.corp.example"));
    }
}
