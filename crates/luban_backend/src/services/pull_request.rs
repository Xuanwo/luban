use luban_domain::{PullRequestCiState, PullRequestState};

pub(super) fn pull_request_ci_state_from_check_buckets<'a>(
    buckets: impl IntoIterator<Item = &'a str>,
) -> Option<PullRequestCiState> {
    let mut any_pending = false;
    let mut any_pass = false;
    let mut any_skip = false;

    for bucket in buckets {
        match bucket {
            "fail" | "cancel" => return Some(PullRequestCiState::Failure),
            "pending" => any_pending = true,
            "pass" => any_pass = true,
            "skipping" => any_skip = true,
            _ => {}
        }
    }

    if any_pending {
        return Some(PullRequestCiState::Pending);
    }
    if any_pass || any_skip {
        return Some(PullRequestCiState::Success);
    }
    None
}

pub(super) fn is_merge_ready(
    pr_state: PullRequestState,
    is_draft: bool,
    merge_state_status: &str,
    review_decision: &str,
    ci_state: Option<PullRequestCiState>,
) -> bool {
    if pr_state != PullRequestState::Open {
        return false;
    }
    if is_draft {
        return false;
    }
    if review_decision != "APPROVED" {
        return false;
    }
    if ci_state != Some(PullRequestCiState::Success) {
        return false;
    }
    matches!(merge_state_status, "CLEAN" | "HAS_HOOKS")
}
