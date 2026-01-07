use super::*;
use luban_domain::PullRequestState;

#[cfg(not(test))]
const PULL_REQUEST_POLL_INTERVAL: Duration = Duration::from_secs(10);
#[cfg(test)]
const PULL_REQUEST_POLL_INTERVAL: Duration = Duration::from_millis(25);

impl LubanRootView {
    fn has_active_workspaces_for_pull_requests(&self) -> bool {
        self.state.projects.iter().any(|project| {
            project.workspaces.iter().any(|workspace| {
                workspace.status == WorkspaceStatus::Active
                    && !(workspace.workspace_name == "main"
                        && workspace.worktree_path == project.path)
            })
        })
    }

    fn ensure_pull_request_refresh_task(&mut self, cx: &mut Context<Self>) {
        if self.pull_request_refresh_task_running {
            return;
        }

        self.pull_request_refresh_task_running = true;
        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    loop {
                        gpui::Timer::after(PULL_REQUEST_POLL_INTERVAL).await;

                        let should_continue = this
                            .update(
                                &mut async_cx,
                                |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                                    let active = view.has_active_workspaces_for_pull_requests();
                                    if !active {
                                        view.pull_request_refresh_task_running = false;
                                        return false;
                                    }

                                    if view.gh_authorized != Some(true) {
                                        view.maybe_check_gh_authorized(view_cx);
                                        return true;
                                    }

                                    view.ensure_workspace_pull_request_numbers(view_cx);
                                    view_cx.notify();
                                    true
                                },
                            )
                            .unwrap_or(false);

                        if !should_continue {
                            break;
                        }
                    }
                }
            },
        )
        .detach();
    }

    pub(super) fn ensure_workspace_pull_request_numbers(&mut self, cx: &mut Context<Self>) {
        let has_active_workspaces = self.has_active_workspaces_for_pull_requests();
        if !has_active_workspaces {
            return;
        }

        if self.gh_authorized != Some(true) {
            self.maybe_check_gh_authorized(cx);
            return;
        }

        self.ensure_pull_request_refresh_task(cx);

        let services = self.services.clone();
        for project in &self.state.projects {
            for workspace in &project.workspaces {
                if workspace.status != WorkspaceStatus::Active {
                    continue;
                }
                if workspace.workspace_name == "main" && workspace.worktree_path == project.path {
                    continue;
                }

                let workspace_id = workspace.id;
                if self.workspace_pull_request_inflight.contains(&workspace_id) {
                    continue;
                }

                if self
                    .workspace_pull_request_numbers
                    .get(&workspace_id)
                    .copied()
                    .flatten()
                    .is_some_and(|info| info.state == PullRequestState::Merged)
                {
                    continue;
                }

                let should_refresh = self
                    .workspace_pull_request_last_checked_at
                    .get(&workspace_id)
                    .map(|last| last.elapsed() >= PULL_REQUEST_POLL_INTERVAL)
                    .unwrap_or(true);
                if !should_refresh {
                    continue;
                }

                self.workspace_pull_request_last_checked_at
                    .insert(workspace_id, Instant::now());
                self.workspace_pull_request_inflight.insert(workspace_id);
                let worktree_path = workspace.worktree_path.clone();
                let services = services.clone();

                cx.spawn(
                    move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                        let mut async_cx = cx.clone();
                        async move {
                            let result = async_cx
                                .background_spawn(async move {
                                    services.gh_pull_request_info(worktree_path)
                                })
                                .await;

                            let pr_number: Option<PullRequestInfo> = result.unwrap_or_default();

                            let _ = this.update(
                                &mut async_cx,
                                |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                                    view.workspace_pull_request_inflight.remove(&workspace_id);
                                    view.workspace_pull_request_last_checked_at
                                        .insert(workspace_id, Instant::now());
                                    let still_active = view.state.projects.iter().any(|project| {
                                        project.workspaces.iter().any(|workspace| {
                                            workspace.id == workspace_id
                                                && workspace.status == WorkspaceStatus::Active
                                                && !(workspace.workspace_name == "main"
                                                    && workspace.worktree_path == project.path)
                                        })
                                    });
                                    if still_active {
                                        view.workspace_pull_request_numbers
                                            .insert(workspace_id, pr_number);
                                    } else {
                                        view.workspace_pull_request_numbers.remove(&workspace_id);
                                        view.workspace_pull_request_last_checked_at
                                            .remove(&workspace_id);
                                    }
                                    view_cx.notify();
                                },
                            );
                        }
                    },
                )
                .detach();
            }
        }
    }

    pub(super) fn maybe_check_gh_authorized(&mut self, cx: &mut Context<Self>) {
        if self.gh_auth_check_inflight {
            return;
        }

        let should_retry = self
            .gh_last_auth_check_at
            .map(|last| last.elapsed() >= Duration::from_secs(10))
            .unwrap_or(true);
        if !should_retry {
            return;
        }

        self.gh_auth_check_inflight = true;
        self.gh_last_auth_check_at = Some(Instant::now());

        let services = self.services.clone();
        cx.spawn(
            move |this: gpui::WeakEntity<LubanRootView>, cx: &mut gpui::AsyncApp| {
                let mut async_cx = cx.clone();
                async move {
                    let result = async_cx
                        .background_spawn(async move { services.gh_is_authorized() })
                        .await;

                    let authorized: bool = result.unwrap_or_default();

                    let _ = this.update(
                        &mut async_cx,
                        |view: &mut LubanRootView, view_cx: &mut Context<LubanRootView>| {
                            view.gh_auth_check_inflight = false;
                            view.gh_authorized = Some(authorized);
                            view.ensure_workspace_pull_request_numbers(view_cx);
                            view_cx.notify();
                        },
                    );
                }
            },
        )
        .detach();
    }
}
