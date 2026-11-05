use super::*;

pub(super) struct LifecycleBindings {
    pub ui_locale: Option<String>,
    pub token: Signal<Option<String>>,
    pub tenant: Signal<Option<String>>,
    pub busy_key: ReadSignal<Option<String>>,
    pub set_busy_key: WriteSignal<Option<String>>,
    pub editor_request: RwSignal<Option<core::EditorRequest>>,
    pub editing_post_id: Signal<Option<String>>,
    pub locale: ReadSignal<String>,
    pub editor_session: RwSignal<core::PostEditorSession>,
    pub form_setters: BlogPostFormSetters,
    pub set_save_notice: WriteSignal<Option<String>>,
    pub set_refresh_nonce: WriteSignal<u64>,
    pub set_submit_error: WriteSignal<Option<WritePathIssue>>,
}

pub(super) struct LifecycleCallbacks {
    pub toggle_publish: Callback<(String, bool, String)>,
    pub toggle_pin: Callback<(String, bool, String)>,
    pub archive_post: Callback<(String, String)>,
    pub restore_post: Callback<(String, String)>,
}

pub(super) fn bind(state: LifecycleBindings) -> LifecycleCallbacks {
    let LifecycleBindings {
        ui_locale,
        token,
        tenant,
        busy_key,
        set_busy_key,
        editor_request,
        editing_post_id,
        locale,
        editor_session,
        form_setters,
        set_save_notice,
        set_refresh_nonce,
        set_submit_error,
    } = state;
    let toggle_publish_locale = ui_locale.clone();
    let toggle_publish = Callback::new(
        move |(post_id, publish, post_locale): (String, bool, String)| {
            if busy_key.get_untracked().is_some() {
                return;
            }
            let token_value = token.get_untracked();
            let tenant_value = tenant.get_untracked();
            let ui_locale = toggle_publish_locale.clone();
            let command =
                core::prepare_blog_post_status_command(post_id, publish, post_locale.as_str());
            let request =
                core::EditorRequest::new(editing_post_id.get_untracked(), locale.get_untracked());
            editor_request.set(Some(request.clone()));
            set_submit_error.set(None);
            set_busy_key.set(Some(command.busy_key.clone()));

            spawn_local(async move {
                let result = match command.operation {
                    core::BlogPostStatusOperation::Publish => {
                        transport::publish_post(
                            token_value.clone(),
                            tenant_value.clone(),
                            command.post_id.clone(),
                        )
                        .await
                    }
                    core::BlogPostStatusOperation::Unpublish => {
                        transport::unpublish_post(
                            token_value.clone(),
                            tenant_value.clone(),
                            command.post_id.clone(),
                        )
                        .await
                    }
                };

                match result {
                    Ok(()) if !request.is_current(editor_request.get_untracked().as_ref()) => {
                        set_refresh_nonce.update(|value| *value += 1);
                        return;
                    }
                    _ if !request.is_current(editor_request.get_untracked().as_ref()) => return,
                    Ok(()) => {
                        let result_view = core::blog_post_mutation_result_view(
                            editing_post_id.get_untracked().as_deref(),
                            command.post_id.as_str(),
                        );
                        if result_view.refresh_posts {
                            set_refresh_nonce.update(|value| *value += 1);
                        }
                        set_save_notice.set(Some(t(
                            ui_locale.as_deref(),
                            "blog.editor.saved",
                            "Post saved.",
                        )));
                        if result_view.reload_selected_post {
                            editor_session.set(crate::core::PostEditorSession::Acknowledged {
                                post_id: command.post_id.clone(),
                            });
                            match transport::fetch_post(
                                token_value,
                                tenant_value,
                                command.post_id.clone(),
                                Some(request.locale.clone()),
                            )
                            .await
                            {
                                _ if !request
                                    .is_current(editor_request.get_untracked().as_ref()) =>
                                {
                                    return;
                                }
                                Ok(Some(post))
                                    if request.accepts_post(
                                        editor_request.get_untracked().as_ref(),
                                        &post,
                                    ) =>
                                {
                                    apply_post_to_form(form_setters, &post)
                                }
                                Ok(_) | Err(_) => editor_session.set(
                                    crate::core::PostEditorSession::ReloadFailed {
                                        post_id: command.post_id.clone(),
                                    },
                                ),
                            }
                        }
                    }
                    Err(err) => {
                        set_submit_error.set(Some(core::blog_post_transport_failure_issue(
                            &t(
                                ui_locale.as_deref(),
                                "blog.error.updateStatus",
                                "Failed to update post status",
                            ),
                            &err.to_string(),
                        )));
                    }
                }

                set_busy_key.set(None);
            });
        },
    );

    let toggle_pin_locale = ui_locale.clone();
    let toggle_pin = Callback::new(move |(post_id, pin, post_locale): (String, bool, String)| {
        if busy_key.get_untracked().is_some() {
            return;
        }
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let ui_locale = toggle_pin_locale.clone();
        let command = core::prepare_blog_post_pin_command(post_id, pin, post_locale.as_str());
        let request =
            core::EditorRequest::new(editing_post_id.get_untracked(), locale.get_untracked());
        editor_request.set(Some(request.clone()));
        set_submit_error.set(None);
        set_busy_key.set(Some(command.busy_key.clone()));

        spawn_local(async move {
            let result = if command.pin {
                transport::pin_post(
                    token_value.clone(),
                    tenant_value.clone(),
                    command.post_id.clone(),
                )
                .await
            } else {
                transport::unpin_post(
                    token_value.clone(),
                    tenant_value.clone(),
                    command.post_id.clone(),
                )
                .await
            };

            match result {
                Ok(()) if !request.is_current(editor_request.get_untracked().as_ref()) => {
                    set_refresh_nonce.update(|value| *value += 1);
                    return;
                }
                _ if !request.is_current(editor_request.get_untracked().as_ref()) => return,
                Ok(()) => {
                    let result_view = core::blog_post_mutation_result_view(
                        editing_post_id.get_untracked().as_deref(),
                        command.post_id.as_str(),
                    );
                    if result_view.refresh_posts {
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    set_save_notice.set(Some(t(
                        ui_locale.as_deref(),
                        "blog.editor.saved",
                        "Post saved.",
                    )));
                    if result_view.reload_selected_post {
                        editor_session.set(crate::core::PostEditorSession::Acknowledged {
                            post_id: command.post_id.clone(),
                        });
                        match transport::fetch_post(
                            token_value,
                            tenant_value,
                            command.post_id.clone(),
                            Some(request.locale.clone()),
                        )
                        .await
                        {
                            _ if !request.is_current(editor_request.get_untracked().as_ref()) => {
                                return;
                            }
                            Ok(Some(post))
                                if request.accepts_post(
                                    editor_request.get_untracked().as_ref(),
                                    &post,
                                ) =>
                            {
                                apply_post_to_form(form_setters, &post)
                            }
                            Ok(_) | Err(_) => {
                                editor_session.set(crate::core::PostEditorSession::ReloadFailed {
                                    post_id: command.post_id.clone(),
                                })
                            }
                        }
                    }
                }
                Err(err) => {
                    set_submit_error.set(Some(core::blog_post_transport_failure_issue(
                        &t(
                            ui_locale.as_deref(),
                            "blog.error.updatePin",
                            "Failed to change post pin state",
                        ),
                        &err.to_string(),
                    )));
                }
            }

            set_busy_key.set(None);
        });
    });

    let archive_post_locale = ui_locale.clone();
    let archive_post = Callback::new(move |(post_id, post_locale): (String, String)| {
        if busy_key.get_untracked().is_some() {
            return;
        }
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let ui_locale = archive_post_locale.clone();
        let command = core::prepare_blog_post_archive_command(post_id, post_locale.as_str());
        let request =
            core::EditorRequest::new(editing_post_id.get_untracked(), locale.get_untracked());
        editor_request.set(Some(request.clone()));
        set_submit_error.set(None);
        set_busy_key.set(Some(command.busy_key.clone()));

        spawn_local(async move {
            match transport::archive_post(
                token_value.clone(),
                tenant_value.clone(),
                command.post_id.clone(),
            )
            .await
            {
                Ok(()) if !request.is_current(editor_request.get_untracked().as_ref()) => {
                    set_refresh_nonce.update(|value| *value += 1);
                    return;
                }
                _ if !request.is_current(editor_request.get_untracked().as_ref()) => return,
                Ok(()) => {
                    let result_view = core::blog_post_mutation_result_view(
                        editing_post_id.get_untracked().as_deref(),
                        command.post_id.as_str(),
                    );
                    if result_view.refresh_posts {
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    set_save_notice.set(Some(t(
                        ui_locale.as_deref(),
                        "blog.editor.saved",
                        "Post saved.",
                    )));
                    if result_view.reload_selected_post {
                        editor_session.set(crate::core::PostEditorSession::Acknowledged {
                            post_id: command.post_id.clone(),
                        });
                        match transport::fetch_post(
                            token_value,
                            tenant_value,
                            command.post_id.clone(),
                            Some(request.locale.clone()),
                        )
                        .await
                        {
                            _ if !request.is_current(editor_request.get_untracked().as_ref()) => {
                                return;
                            }
                            Ok(Some(post))
                                if request.accepts_post(
                                    editor_request.get_untracked().as_ref(),
                                    &post,
                                ) =>
                            {
                                apply_post_to_form(form_setters, &post)
                            }
                            Ok(_) | Err(_) => {
                                editor_session.set(crate::core::PostEditorSession::ReloadFailed {
                                    post_id: command.post_id.clone(),
                                })
                            }
                        }
                    }
                }
                Err(err) => {
                    set_submit_error.set(Some(core::blog_post_transport_failure_issue(
                        &t(
                            ui_locale.as_deref(),
                            "blog.error.archivePost",
                            "Failed to archive post",
                        ),
                        &err.to_string(),
                    )));
                }
            }

            set_busy_key.set(None);
        });
    });

    let restore_post_locale = ui_locale.clone();
    let restore_post = Callback::new(move |(post_id, post_locale): (String, String)| {
        if busy_key.get_untracked().is_some() {
            return;
        }
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let ui_locale = restore_post_locale.clone();
        let command = core::prepare_blog_post_restore_command(post_id, post_locale.as_str());
        let request =
            core::EditorRequest::new(editing_post_id.get_untracked(), locale.get_untracked());
        editor_request.set(Some(request.clone()));
        set_submit_error.set(None);
        set_busy_key.set(Some(command.busy_key.clone()));

        spawn_local(async move {
            match transport::restore_post(
                token_value.clone(),
                tenant_value.clone(),
                command.post_id.clone(),
            )
            .await
            {
                Ok(()) if !request.is_current(editor_request.get_untracked().as_ref()) => {
                    set_refresh_nonce.update(|value| *value += 1);
                    return;
                }
                _ if !request.is_current(editor_request.get_untracked().as_ref()) => return,
                Ok(()) => {
                    let result_view = core::blog_post_mutation_result_view(
                        editing_post_id.get_untracked().as_deref(),
                        command.post_id.as_str(),
                    );
                    if result_view.refresh_posts {
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    set_save_notice.set(Some(t(
                        ui_locale.as_deref(),
                        "blog.editor.saved",
                        "Post saved.",
                    )));
                    if result_view.reload_selected_post {
                        editor_session.set(crate::core::PostEditorSession::Acknowledged {
                            post_id: command.post_id.clone(),
                        });
                        match transport::fetch_post(
                            token_value,
                            tenant_value,
                            command.post_id.clone(),
                            Some(request.locale.clone()),
                        )
                        .await
                        {
                            _ if !request.is_current(editor_request.get_untracked().as_ref()) => {
                                return;
                            }
                            Ok(Some(post))
                                if request.accepts_post(
                                    editor_request.get_untracked().as_ref(),
                                    &post,
                                ) =>
                            {
                                apply_post_to_form(form_setters, &post)
                            }
                            Ok(_) | Err(_) => {
                                editor_session.set(crate::core::PostEditorSession::ReloadFailed {
                                    post_id: command.post_id.clone(),
                                })
                            }
                        }
                    }
                }
                Err(err) => {
                    set_submit_error.set(Some(core::blog_post_transport_failure_issue(
                        &t(
                            ui_locale.as_deref(),
                            "blog.error.restorePost",
                            "Failed to restore post",
                        ),
                        &err.to_string(),
                    )));
                }
            }

            set_busy_key.set(None);
        });
    });

    LifecycleCallbacks {
        toggle_publish,
        toggle_pin,
        archive_post,
        restore_post,
    }
}
