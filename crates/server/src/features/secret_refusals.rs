//! Vault refusals as protocol errors. Messages and data carry scopes, names and request ids, never a value.

use massorca_core_vault::{Scope, ScopeError, ScopedName, VaultError};
use massorca_protocol::{ErrorBody, codes};
use serde_json::{Value, json};

/// Parses untrusted scope text; the refusal never repeats it.
pub(crate) fn parse_scope(text: &str) -> Result<Scope, ErrorBody> {
    text.parse()
        .map_err(|error: ScopeError| ErrorBody::new(codes::SECRET_INVALID_SCOPE, error.to_string()))
}

/// The protocol error for a vault refusal.
pub(crate) fn refusal(error: VaultError) -> ErrorBody {
    let message = error.to_string();
    let (code, data) = match error {
        VaultError::Locked(_) => (codes::SECRET_VAULT_LOCKED, None),
        VaultError::KeyMismatch { unreadable } => (
            codes::SECRET_VAULT_LOCKED,
            Some(json!({ "reason": "key_mismatch", "unreadable": scoped(unreadable) })),
        ),
        VaultError::NameClash { clash } => (
            codes::SECRET_NAME_CLASH,
            Some(json!({ "names": scoped(clash) })),
        ),
        VaultError::NotFound { scope, name } => (codes::SECRET_NOT_FOUND, Some(named(scope, name))),
        VaultError::NameInUse { scope, name } => {
            (codes::SECRET_NAME_IN_USE, Some(named(scope, name)))
        }
        VaultError::Linked { scope, name, owner } => (
            codes::SECRET_LINKED,
            Some(json!({ "scope": scope, "name": name, "owner": owner })),
        ),
        VaultError::HasLinks {
            scope,
            name,
            linked,
        } => (
            codes::SECRET_HAS_LINKS,
            Some(json!({ "scope": scope, "name": name, "linked": linked })),
        ),
        VaultError::InvalidName => (codes::SECRET_INVALID_NAME, None),
        VaultError::ReservedName { name } => {
            (codes::SECRET_RESERVED_NAME, Some(json!({ "name": name })))
        }
        VaultError::InvalidScope => (codes::SECRET_INVALID_SCOPE, None),
        VaultError::TooLarge => (codes::SECRET_TOO_LARGE, None),
        VaultError::InvalidService | VaultError::InvalidServiceField(_) => {
            (codes::REQUEST_INVALID_PARAMS, None)
        }
        VaultError::InvalidReason => (codes::SECRET_INVALID_REASON, None),
        VaultError::InvalidSession => (codes::SECRET_INVALID_SESSION, None),
        VaultError::NameCaseClash {
            scope,
            name,
            other_scope,
            other,
        } => (
            codes::SECRET_NAME_IN_USE,
            Some(json!({
                "scope": scope,
                "name": name,
                "other_scope": other_scope,
                "other": other
            })),
        ),
        VaultError::VaultNotEmpty { count } => (
            codes::SECRET_VAULT_NOT_EMPTY,
            Some(json!({ "count": count })),
        ),
        VaultError::RequestLimit(_) => (codes::SECRET_REQUEST_LIMIT, None),
        VaultError::Unreadable { scope, name } => {
            (codes::SECRET_UNREADABLE, Some(named(scope, name)))
        }
        VaultError::RequestNotFound(id) => (
            codes::SECRET_REQUEST_NOT_FOUND,
            Some(json!({ "request": id })),
        ),
        VaultError::RequestResolved { id, state } => (
            codes::SECRET_REQUEST_RESOLVED,
            Some(json!({ "request": id, "state": state })),
        ),
        VaultError::Key(_) | VaultError::Cipher | VaultError::Store(_) => (codes::INTERNAL, None),
    };
    ErrorBody {
        code: code.to_owned(),
        message,
        data,
    }
}

fn scoped(list: Vec<ScopedName>) -> Value {
    list.into_iter()
        .map(|entry| named(entry.scope, entry.name))
        .collect()
}

fn named(scope: String, name: String) -> Value {
    json!({ "scope": scope, "name": name })
}

#[cfg(test)]
mod tests {
    use super::{parse_scope, refusal};
    use massorca_core_vault::VaultError;
    use massorca_protocol::codes;

    #[test]
    fn has_links_lists_the_linked_scopes_in_data() {
        let body = refusal(VaultError::HasLinks {
            scope: "global".to_owned(),
            name: "TOKEN".to_owned(),
            linked: vec!["project:a".to_owned(), "team:b".to_owned()],
        });
        assert_eq!(body.code, codes::SECRET_HAS_LINKS);
        let data = body.data.expect("data");
        assert_eq!(data["linked"], serde_json::json!(["project:a", "team:b"]));
    }

    #[test]
    fn an_unreadable_row_names_its_scope_and_name_and_is_not_internal() {
        let body = refusal(VaultError::Unreadable {
            scope: "project:a".to_owned(),
            name: "TOKEN".to_owned(),
        });
        assert_eq!(body.code, codes::SECRET_UNREADABLE);
        let data = body.data.expect("data");
        assert_eq!(data["scope"], "project:a");
        assert_eq!(data["name"], "TOKEN");
    }

    #[test]
    fn an_invalid_session_has_its_own_code() {
        assert_eq!(
            refusal(VaultError::InvalidSession).code,
            codes::SECRET_INVALID_SESSION
        );
    }

    #[test]
    fn a_bad_scope_is_invalid_scope_without_echoing_the_text() {
        let body = parse_scope("nowhere-secret-text").expect_err("refused");
        assert_eq!(body.code, codes::SECRET_INVALID_SCOPE);
        assert!(!body.message.contains("nowhere-secret-text"));
    }
}
