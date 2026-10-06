// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! SSRF guard for per-request indexer overrides, ported from the TS
//! service's `IndexerEndpointPolicy`: reject embedded credentials and
//! (unless explicitly allowed for dev) loopback / private / link-local
//! hosts. The service's own configured default endpoint is exempt —
//! the policy applies to caller-supplied overrides only.

use std::net::IpAddr;

use url::{Host, Url};

/// Validate a caller-supplied indexer override URL.
///
/// Returns the normalized URL string, or a rejection reason.
pub fn validate_override(raw: &str, allow_private: bool) -> Result<String, String> {
    let url = Url::parse(raw).map_err(|e| format!("indexerUrl is not a valid URL: {e}"))?;
    match url.scheme() {
        "http" | "https" => {}
        other => return Err(format!("indexerUrl scheme '{other}' is not allowed")),
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("indexerUrl must not embed credentials".into());
    }
    let Some(host) = url.host() else {
        return Err("indexerUrl has no host".into());
    };
    if !allow_private && is_private_host(&host) {
        return Err("indexerUrl must not target private/loopback hosts".into());
    }
    Ok(url.to_string())
}

fn is_private_host(host: &Host<&str>) -> bool {
    match host {
        Host::Domain(d) => {
            let d = d.to_ascii_lowercase();
            d == "localhost" || d.ends_with(".localhost") || d.ends_with(".local") || d.ends_with(".internal")
        }
        Host::Ipv4(ip) => {
            ip.is_loopback() || ip.is_private() || ip.is_link_local() || ip.is_unspecified() || ip.is_broadcast()
        }
        Host::Ipv6(ip) => {
            let ip = IpAddr::V6(*ip);
            match ip {
                IpAddr::V6(v6) => {
                    v6.is_loopback() || v6.is_unspecified() || v6.is_unique_local() || v6.is_unicast_link_local()
                }
                IpAddr::V4(_) => unreachable!(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_public_https() {
        assert!(validate_override("https://indexer.mainnet.midnight.network/api/v4/graphql", false).is_ok());
    }

    #[test]
    fn rejects_loopback_when_private_disallowed() {
        assert!(validate_override("http://127.0.0.1:8088/api/v3/graphql", false).is_err());
        assert!(validate_override("http://localhost:8088/api/v3/graphql", false).is_err());
        assert!(validate_override("http://10.0.0.5/graphql", false).is_err());
        assert!(validate_override("http://169.254.1.1/graphql", false).is_err());
        assert!(validate_override("http://[::1]:8088/graphql", false).is_err());
    }

    #[test]
    fn allows_loopback_in_dev_mode() {
        assert!(validate_override("http://127.0.0.1:8088/api/v3/graphql", true).is_ok());
    }

    #[test]
    fn rejects_credentials_and_bad_schemes() {
        assert!(validate_override("http://user:pw@example.com/graphql", true).is_err());
        assert!(validate_override("ftp://example.com/graphql", true).is_err());
        assert!(validate_override("not a url", true).is_err());
    }
}
