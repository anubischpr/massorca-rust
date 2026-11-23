//! What a package problem means in the Marketplace (decision 0062). A package whose files no longer match what the
//! lockfile recorded is `extension.tampered`: it is shown as a danger, cannot be installed again or turned on, and
//! can still be uninstalled. Any other problem is shown as a warning with the service's own sentence.

use massorca_protocol::codes;
use massorca_protocol::methods::extensions::ItemProblem;

use super::types::MarketplaceItem;

/// The tone of the alert a package problem shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProblemTone {
    Danger,
    Warning,
}

/// The alert an installed package with a problem shows on its page and its card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProblemAlert {
    pub tone: ProblemTone,
    pub message: String,
}

/// An action the Marketplace offers on a package that a problem may refuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackageAction {
    Install,
    Enable,
}

/// The alert for an item's package problem, or None when the package has none.
pub fn problem_alert(item: &MarketplaceItem) -> Option<ProblemAlert> {
    let problem = item.extension.as_deref()?.problem.as_ref()?;
    Some(ProblemAlert {
        tone: tone_of(problem),
        message: sentence_of(problem),
    })
}

/// The sentence that refuses an action on a package, or None when the action may go on. Only a tampered package is
/// refused; uninstalling is never refused here.
pub fn package_refusal(item: &MarketplaceItem, action: PackageAction) -> Option<String> {
    let problem = item.extension.as_deref()?.problem.as_ref()?;
    if problem.code != codes::EXTENSION_TAMPERED {
        return None;
    }
    Some(match action {
        PackageAction::Install => "This package's files changed on this computer, so it cannot be installed again. Uninstall it first.".to_owned(),
        PackageAction::Enable => "This package's files changed on this computer, so it cannot be turned on. Uninstall it and install it again.".to_owned(),
    })
}

fn tone_of(problem: &ItemProblem) -> ProblemTone {
    if problem.code == codes::EXTENSION_TAMPERED {
        ProblemTone::Danger
    } else {
        ProblemTone::Warning
    }
}

fn sentence_of(problem: &ItemProblem) -> String {
    if problem.code == codes::EXTENSION_TAMPERED {
        "Its files changed on this computer after it was installed, so it cannot run. Uninstall it and install it again."
            .to_owned()
    } else {
        problem.message.clone()
    }
}
