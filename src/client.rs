//! gRPC connection helper for the `ec` Admin service.
//!
//! Wraps the generated [`AdminClient`] and injects the optional
//! `authorization: Bearer <token>` metadata expected by the `ec` daemon when
//! `EC_ADMIN_TOKEN` is configured server-side.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use tonic::metadata::{Ascii, MetadataValue};
use tonic::transport::Channel;
use tonic::Request;

use crate::proto::admin_client::AdminClient;

/// Give up on an unresponsive endpoint rather than hanging forever: a routable
/// but silently-dropping address (firewall, hung proxy) would otherwise never
/// return, so the `Unavailable` hint would never be reached.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Extract the host (without port) from a `scheme://host[:port][/path]` URL.
fn host_of(url: &str) -> Option<&str> {
    let authority = url.split("://").nth(1)?.split('/').next()?;
    // IPv6 literals are bracketed and contain colons, so handle them first.
    if authority.starts_with('[') {
        let end = authority.find(']')?;
        return Some(&authority[..=end]);
    }
    authority.split(':').next()
}

/// Whether traffic to `host` stays on the local machine, where the lack of
/// transport encryption does not expose credentials to the network.
fn is_local_host(host: &str) -> bool {
    matches!(host, "localhost" | "::1" | "[::1]") || host.starts_with("127.")
}

/// A connected Admin client plus the optional pre-parsed auth header.
pub struct EcClient {
    inner: AdminClient<Channel>,
    auth: Option<MetadataValue<Ascii>>,
}

impl EcClient {
    /// Connect to `server` (e.g. `http://127.0.0.1:50051`). When `token` is a
    /// non-empty string, every request carries `authorization: Bearer <token>`.
    pub async fn connect(server: &str, token: Option<&str>) -> Result<Self> {
        // No TLS feature is compiled in, so tonic would happily send an
        // `https://` URL as plaintext HTTP/2. Fail loudly instead of silently
        // downgrading a connection the operator believes is encrypted.
        if server.starts_with("https://") {
            bail!(
                "eccli does not support TLS yet, and would send traffic to {server} in cleartext\
                 \n  hint: use http:// over a trusted network, or tunnel it (e.g. ssh -L)"
            );
        }
        if let Some(host) = host_of(server) {
            if !is_local_host(host) {
                eprintln!(
                    "⚠️  Warning: {host} is not local and the connection is unencrypted — \
                     the admin token and any generated registration tokens are sent in cleartext."
                );
            }
        }

        let auth = build_auth(token)?;
        let channel = Channel::from_shared(server.to_string())
            .with_context(|| format!("invalid server URL: {server}"))?
            .connect_timeout(CONNECT_TIMEOUT)
            .connect()
            .await
            .with_context(|| format!("failed to connect to ec gRPC server at {server}"))?;
        Ok(Self {
            inner: AdminClient::new(channel),
            auth,
        })
    }

    /// Wrap a message in a [`Request`], attaching the auth header when present.
    pub fn request<T>(&self, message: T) -> Request<T> {
        let mut req = Request::new(message);
        if let Some(value) = &self.auth {
            req.metadata_mut().insert("authorization", value.clone());
        }
        req
    }

    /// Mutable access to the underlying generated client for issuing RPCs.
    pub fn inner(&mut self) -> &mut AdminClient<Channel> {
        &mut self.inner
    }
}

/// Parse an optional bearer token into an HTTP-header metadata value.
///
/// Returns `Ok(None)` when the token is absent or empty (auth disabled).
pub fn build_auth(token: Option<&str>) -> Result<Option<MetadataValue<Ascii>>> {
    match token {
        Some(t) if !t.is_empty() => {
            let value: MetadataValue<Ascii> = format!("Bearer {t}")
                .parse()
                .context("admin token is not a valid HTTP header value")?;
            Ok(Some(value))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_token_yields_no_auth() {
        assert!(build_auth(None).unwrap().is_none());
        assert!(build_auth(Some("")).unwrap().is_none());
    }

    #[test]
    fn token_becomes_bearer_header() {
        let v = build_auth(Some("s3cret")).unwrap().unwrap();
        assert_eq!(v.to_str().unwrap(), "Bearer s3cret");
    }

    #[test]
    fn host_is_extracted_without_port_or_path() {
        assert_eq!(host_of("http://127.0.0.1:50051"), Some("127.0.0.1"));
        assert_eq!(
            host_of("http://ec.example.org/admin"),
            Some("ec.example.org")
        );
        assert_eq!(host_of("http://[::1]:50051"), Some("[::1]"));
        assert_eq!(host_of("not-a-url"), None);
    }

    #[test]
    fn loopback_hosts_are_local() {
        assert!(is_local_host("127.0.0.1"));
        assert!(is_local_host("127.1.2.3"));
        assert!(is_local_host("localhost"));
        assert!(is_local_host("[::1]"));
        assert!(!is_local_host("ec.example.org"));
        assert!(!is_local_host("10.0.0.5"));
    }

    #[tokio::test]
    async fn https_is_rejected_rather_than_downgraded() {
        // No TLS feature is compiled in, so this must fail loudly instead of
        // sending the admin token in cleartext.
        // Matched rather than `unwrap_err`ed: `EcClient` intentionally has no
        // `Debug` impl, so the auth token cannot leak through `{:?}`.
        let msg = match EcClient::connect("https://ec.example.org:50051", Some("s3cret")).await {
            Ok(_) => panic!("https:// must be rejected"),
            Err(e) => e.to_string(),
        };
        assert!(msg.contains("does not support TLS"), "got: {msg}");
        assert!(!msg.contains("s3cret"), "token must not leak into errors");
    }
}
