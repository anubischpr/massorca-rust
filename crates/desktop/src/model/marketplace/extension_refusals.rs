//! The sentences the Marketplace shows for an extension refusal (decision 0062). A code this list names gets its own
//! sentence; any other refusal keeps the service's sentence, which the CLI shows too.

/// The sentence for a refused extension call, from the service's code and its own sentence.
pub fn extension_refusal_text(code: Option<&str>, service_sentence: &str) -> String {
    let text = match code {
        Some("extension.consent_required") => {
            "Review the package and accept it before it is installed."
        }
        Some("extension.consent_mismatch") => {
            "The package changed after you reviewed it. Review it again before installing."
        }
        Some("extension.installer_consent_required") => {
            "Installing this package runs a vendor installer. Accept that step too."
        }
        Some("extension.busy") => {
            "This package is busy. Wait for its install or removal to finish."
        }
        Some("extension.needs_person") => {
            "A person has to do this; an agent cannot run a vendor program."
        }
        Some("extension.vendor_command_refused") => {
            "This package asks for a vendor program, which only MassOrca's own packages may run."
        }
        Some("extension.tampered") => {
            "This package's files changed on this computer. Remove it and install it again."
        }
        Some("extension.reserved_publisher") => "This package uses a name reserved for MassOrca.",
        Some("extension.not_found") => "This package is not in the catalogue.",
        Some("extension.already_installed") => "This package is already installed.",
        Some("extension.kind_conflict") => "Another package already provides this agent.",
        _ => return service_sentence.to_owned(),
    };
    text.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_named_code_gets_its_own_sentence_and_others_keep_the_service_words() {
        assert_eq!(
            extension_refusal_text(Some("extension.consent_mismatch"), "service words"),
            "The package changed after you reviewed it. Review it again before installing."
        );
        assert_eq!(
            extension_refusal_text(Some("extension.verify_failed"), "The CLI did not verify."),
            "The CLI did not verify."
        );
        assert_eq!(extension_refusal_text(None, "Plain."), "Plain.");
    }
}
