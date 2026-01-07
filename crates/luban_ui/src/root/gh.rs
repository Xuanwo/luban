use super::*;

impl LubanRootView {
    pub(super) fn ensure_workspace_pull_request_numbers(&mut self, cx: &mut Context<Self>) {
        let has_active_workspaces = self.state.projects.iter().any(|project| {
            project.workspaces.iter().any(|workspace| {
                workspace.status == WorkspaceStatus::Active
                    && !(workspace.workspace_name == "main"
                        && workspace.worktree_path == project.path)
            })
        });
        if !has_active_workspaces {
            return;
        }

        if self.gh_authorized != Some(true) {
            self.maybe_check_gh_authorized(cx);
            return;
        }

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
                if self
                    .workspace_pull_request_numbers
                    .contains_key(&workspace_id)
                    || self.workspace_pull_request_inflight.contains(&workspace_id)
                {
                    continue;
                }

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
