//! The one error that crosses the binding (decision 0080): a flat case per thing the app does differently, each with a
//! sentence it may show as is.

use massorca_client::{ClientError, IdentityMismatch, TrustedFrom};
use massorca_protocol::codes;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum FfiError {
    /// The host does not answer now; a retry can help.
    #[error("{message}")]
    Unreachable { message: String },
    /// The host presents another server key than the one paired with (REM-04). Never retried; pairing again is the fix.
    /// `expected` is the trusted key, `actual` the presented one when its certificate could be read.
    #[error("{message}")]
    IdentityMismatch {
        message: String,
        expected: String,
        actual: Option<String>,
    },
    /// The host no longer accepts this phone: it was revoked or forgotten there.
    #[error("{message}")]
    NotPaired { message: String },
    /// The host and the app speak no common protocol version; `supported_*` is the host's range when it said.
    #[error("{message}")]
    ProtocolMismatch {
        message: String,
        supported_min: Option<u32>,
        supported_max: Option<u32>,
    },
    /// The text is not a pairing code, it expired, or it was already used.
    #[error("{message}")]
    InvalidOffer { message: String },
    #[error("{message}")]
    HostExists { message: String },
    #[error("{message}")]
    HostNotFound { message: String },
    #[error("{message}")]
    InvalidHostName { message: String },
    /// Any other refusal of the service, with its own code and sentence.
    #[error("{message}")]
    Remote { code: String, message: String },
    /// The phone's store failed: the pairing key could not be read or written.
    #[error("{message}")]
    Storage { message: String },
    /// Unexpected data from the host.
    #[error("{message}")]
    Internal { message: String },
}

impl FfiError {
    /// A stable code per case, for logs and tests; `Remote` keeps the service's own.
    pub fn code(&self) -> &str {
        match self {
            Self::Unreachable { .. } => "unreachable",
            Self::IdentityMismatch { .. } => "identity_mismatch",
            Self::NotPaired { .. } => "not_paired",
            Self::ProtocolMismatch { .. } => "protocol_mismatch",
            Self::InvalidOffer { .. } => "invalid_offer",
            Self::HostExists { .. } => "host_exists",
            Self::HostNotFound { .. } => "host_not_found",
            Self::InvalidHostName { .. } => "invalid_host_name",
            Self::Remote { code, .. } => code,
            Self::Storage { .. } => "storage",
            Self::Internal { .. } => "internal",
        }
    }

    /// Whether trying again can help, so the app picks "Retry" over "Pair again" without reading the text.
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Unreachable { .. } => true,
            Self::Remote { code, .. } => code == codes::SERVICE_SHUTTING_DOWN,
            _ => false,
        }
    }

    /// The case for a client failure while talking to `host`, the name the app shows for it.
    pub fn from_client(error: ClientError, host: &str) -> Self {
        match error {
            ClientError::HostUnreachable(_)
            | ClientError::Unavailable(_)
            | ClientError::Disconnected => unreachable(host),
            ClientError::ServerIdentityMismatch(mismatch) => identity_mismatch(*mismatch),
            ClientError::Remote(body) => from_body(body.code, body.message, None, host),
            ClientError::Handshake { error, supported } => from_body(
                error.code,
                error.message,
                supported.map(|range| (range.min, range.max)),
                host,
            ),
            error @ ClientError::HostExists(_) => Self::HostExists {
                message: sentence(&error.to_string()),
            },
            error @ ClientError::HostNotFound(_) => Self::HostNotFound {
                message: sentence(&error.to_string()),
            },
            error @ ClientError::InvalidHostName(_) => Self::InvalidHostName {
                message: sentence(&error.to_string()),
            },
            ClientError::Protocol(_) => Self::Internal {
                message: format!("Unexpected data from {host}."),
            },
            // The phone has no local service: what is left is its store.
            error => Self::Storage {
                message: sentence(&format!(
                    "the pairing key could not be read or saved: {error}"
                )),
            },
        }
    }
}

/// The phone's own fix: it forgets the host and scans a new code; the CLI's commands do not apply here.
fn identity_mismatch(mismatch: IdentityMismatch) -> FfiError {
    let host = &mismatch.host;
    let actual = mismatch
        .actual
        .as_deref()
        .unwrap_or("a certificate that could not be read");
    let message = match mismatch.trusted_from {
        TrustedFrom::Pairing => format!(
            "{host} presented a different server key than the one paired with (paired {}, now {actual}). If that              server was reinstalled or its key replaced, forget {host} on this phone and pair again with a new code.              Otherwise do not connect: something may be between this phone and the server.",
            mismatch.expected
        ),
        TrustedFrom::Offer => format!(
            "The server presented a different key than the pairing code names (code {}, server {actual}). Nothing              was paired. Make a new code on that server and scan it.",
            mismatch.expected
        ),
    };
    FfiError::IdentityMismatch {
        message,
        expected: mismatch.expected,
        actual: mismatch.actual,
    }
}

fn unreachable(host: &str) -> FfiError {
    FfiError::Unreachable {
        message: format!("{host} can't be reached."),
    }
}

fn from_body(code: String, message: String, supported: Option<(u32, u32)>, host: &str) -> FfiError {
    match code.as_str() {
        codes::AUTH_DEVICE_REVOKED | codes::AUTH_UNKNOWN_DEVICE | codes::AUTH_REQUIRED => {
            FfiError::NotPaired {
                message: format!("{host} no longer trusts this phone. Pair again."),
            }
        }
        codes::PROTOCOL_UNSUPPORTED_VERSION => FfiError::ProtocolMismatch {
            message: sentence(&message),
            supported_min: supported.map(|range| range.0),
            supported_max: supported.map(|range| range.1),
        },
        // The handshake never got an answer: the host is not there.
        codes::PROTOCOL_HANDSHAKE => unreachable(host),
        codes::PAIRING_INVALID | codes::PAIRING_EXPIRED => FfiError::InvalidOffer {
            message:
                "Not a valid pairing code, or it was already used. Make a new one on the host."
                    .to_owned(),
        },
        _ => FfiError::Remote { code, message },
    }
}

/// The text with its first letter capitalised and a closing full stop.
fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    let mut out: String = chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default();
    if !out.ends_with('.') {
        out.push('.');
    }
    out
}

#[cfg(test)]
mod tests {
    use std::io;

    use massorca_client::ClientError;
    use massorca_protocol::{ErrorBody, ProtocolRange, codes};

    use super::FfiError;

    fn of(error: ClientError) -> FfiError {
        FfiError::from_client(error, "VPS")
    }

    fn handshake(code: &str, supported: Option<ProtocolRange>) -> ClientError {
        ClientError::Handshake {
            error: Box::new(ErrorBody::new(code, "refused")),
            supported,
        }
    }

    #[test]
    fn a_host_that_does_not_answer_is_unreachable_and_worth_a_retry() {
        for error in [
            ClientError::HostUnreachable("timed out".into()),
            ClientError::Disconnected,
            ClientError::Unavailable(io::Error::other("refused")),
            handshake(codes::PROTOCOL_HANDSHAKE, None),
        ] {
            let mapped = of(error);
            assert_eq!(
                mapped,
                FfiError::Unreachable {
                    message: "VPS can't be reached.".into()
                }
            );
            assert!(mapped.is_transient());
        }
    }

    #[test]
    fn a_changed_server_key_is_an_identity_mismatch_and_never_retried() {
        let changed = massorca_client::IdentityMismatch {
            host: "VPS".into(),
            trusted_from: massorca_client::TrustedFrom::Pairing,
            expected: "sha256:old".into(),
            actual: Some("sha256:new".into()),
        };
        let mapped = of(ClientError::ServerIdentityMismatch(Box::new(changed)));
        assert_eq!(mapped.code(), "identity_mismatch");
        assert!(!mapped.is_transient());
        let FfiError::IdentityMismatch {
            message,
            expected,
            actual,
        } = mapped
        else {
            panic!("an identity mismatch");
        };
        assert_eq!(expected, "sha256:old");
        assert_eq!(actual.as_deref(), Some("sha256:new"));
        assert!(message.contains("forget VPS on this phone"), "{message}");
        assert!(!message.contains("massorca host remove"), "{message}");
    }

    #[test]
    fn a_revoked_or_unknown_device_must_pair_again() {
        for code in [
            codes::AUTH_DEVICE_REVOKED,
            codes::AUTH_UNKNOWN_DEVICE,
            codes::AUTH_REQUIRED,
        ] {
            let mapped = of(handshake(code, None));
            assert_eq!(
                mapped,
                FfiError::NotPaired {
                    message: "VPS no longer trusts this phone. Pair again.".into()
                }
            );
            assert!(!mapped.is_transient());
        }
        let in_call = of(ClientError::Remote(ErrorBody::new(
            codes::AUTH_DEVICE_REVOKED,
            "revoked",
        )));
        assert_eq!(in_call.code(), "not_paired");
    }

    #[test]
    fn a_version_refusal_carries_the_hosts_range() {
        let mapped = of(handshake(
            codes::PROTOCOL_UNSUPPORTED_VERSION,
            Some(ProtocolRange { min: 2, max: 3 }),
        ));
        assert_eq!(
            mapped,
            FfiError::ProtocolMismatch {
                message: "Refused.".into(),
                supported_min: Some(2),
                supported_max: Some(3),
            }
        );
    }

    #[test]
    fn a_used_or_expired_offer_is_an_invalid_offer() {
        for code in [codes::PAIRING_INVALID, codes::PAIRING_EXPIRED] {
            assert_eq!(of(handshake(code, None)).code(), "invalid_offer");
        }
    }

    #[test]
    fn host_name_errors_keep_their_own_case_and_sentence() {
        assert_eq!(
            of(ClientError::HostExists("vps".into())),
            FfiError::HostExists {
                message: "A host named \"vps\" already exists.".into()
            }
        );
        assert_eq!(
            of(ClientError::HostNotFound("vps".into())).code(),
            "host_not_found"
        );
        assert_eq!(
            of(ClientError::InvalidHostName("V!".into())).code(),
            "invalid_host_name"
        );
    }

    #[test]
    fn any_other_refusal_keeps_the_services_code_and_only_shutting_down_is_retried() {
        let mapped = of(ClientError::Remote(ErrorBody::new(
            "workspace.not_found",
            "no such workspace",
        )));
        assert_eq!(
            mapped,
            FfiError::Remote {
                code: "workspace.not_found".into(),
                message: "no such workspace".into()
            }
        );
        assert!(!mapped.is_transient());
        let leaving = of(ClientError::Remote(ErrorBody::new(
            codes::SERVICE_SHUTTING_DOWN,
            "bye",
        )));
        assert!(leaving.is_transient());
    }

    #[test]
    fn store_failures_are_storage_and_bad_data_is_internal() {
        assert_eq!(
            of(ClientError::Io(io::Error::other("sealed key unreadable"))).code(),
            "storage"
        );
        assert_eq!(
            of(ClientError::Protocol("bad frame".into())),
            FfiError::Internal {
                message: "Unexpected data from VPS.".into()
            }
        );
    }
}
