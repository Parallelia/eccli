//! gRPC connection helper for the `ec` Admin service.
//!
//! Wraps the generated [`AdminClient`] and injects the optional
//! `authorization: Bearer <token>` metadata expected by the `ec` daemon when
//! `EC_ADMIN_TOKEN` is configured server-side.

use anyhow::{Context, Result};
use tonic::metadata::{Ascii, MetadataValue};
use tonic::transport::Channel;
use tonic::Request;

use crate::proto::admin_client::AdminClient;

/// A connected Admin client plus the optional pre-parsed auth header.
pub struct EcClient {
    inner: AdminClient<Channel>,
    auth: Option<MetadataValue<Ascii>>,
}

impl EcClient {
    /// Connect to `server` (e.g. `http://127.0.0.1:50051`). When `token` is a
    /// non-empty string, every request carries `authorization: Bearer <token>`.
    pub async fn connect(server: &str, token: Option<&str>) -> Result<Self> {
        let auth = build_auth(token)?;
        let channel = Channel::from_shared(server.to_string())
            .with_context(|| format!("invalid server URL: {server}"))?
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
}
