//! Project sidebar: list, load, delete, and rename saved projects.

#[cfg(target_arch = "wasm32")]
mod inner {
    use leptos::prelude::*;

    use super::super::storage::{
        delete_project, format_relative_time, list_projects, SavedProject,
    };

    /// Sidebar listing saved projects with load/delete actions.
    #[component]
    pub fn ProjectSidebar(
        on_load: Callback<SavedProject>,
        on_new: Callback<()>,
        active_project: ReadSignal<String>,
        set_active_project: WriteSignal<String>,
    ) -> impl IntoView {
        // Refresh trigger — bump to re-read LocalStorage.
        let (refresh, set_refresh) = signal(0_u32);

        let projects = move || {
            // Subscribe to the refresh signal so we re-read on change.
            refresh.get();
            list_projects()
        };

        let now_ms = move || js_sys::Date::now();

        view! {
            <div class="project-sidebar">
                <div class="sidebar-header">
                    <span class="sidebar-title">"Projects"</span>
                    <button
                        class="btn btn-sm btn-new-project"
                        on:click=move |_| {
                            on_new.run(());
                            set_refresh.update(|n| *n += 1);
                        }
                    >
                        "New"
                    </button>
                </div>

                <div class="sidebar-active-name">
                    <label>"Name:"</label>
                    <input
                        type="text"
                        class="project-name-input"
                        prop:value=move || active_project.get()
                        on:input=move |ev| {
                            use wasm_bindgen::JsCast;
                            if let Some(input) = ev
                                .target()
                                .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                            {
                                set_active_project.set(input.value());
                            }
                        }
                    />
                </div>

                <ul class="project-list">
                    {move || {
                        let current_now = now_ms();
                        projects()
                            .into_iter()
                            .map(|(name, ts)| {
                                let name_load = name.clone();
                                let name_delete = name.clone();
                                let relative = format_relative_time(ts, current_now);
                                view! {
                                    <li class="project-item">
                                        <div class="project-info">
                                            <span class="project-name">{name.clone()}</span>
                                            <span class="project-time">{relative}</span>
                                        </div>
                                        <div class="project-actions">
                                            <button
                                                class="btn btn-xs btn-load"
                                                on:click={
                                                    let name_load = name_load.clone();
                                                    move |_| {
                                                        if let Some(proj) =
                                                            super::super::storage::load_project(&name_load)
                                                        {
                                                            on_load.run(proj);
                                                        }
                                                        set_refresh.update(|n| *n += 1);
                                                    }
                                                }
                                            >
                                                "Load"
                                            </button>
                                            <button
                                                class="btn btn-xs btn-delete"
                                                on:click={
                                                    let name_delete = name_delete.clone();
                                                    move |_| {
                                                        delete_project(&name_delete);
                                                        set_refresh.update(|n| *n += 1);
                                                    }
                                                }
                                            >
                                                "Delete"
                                            </button>
                                        </div>
                                    </li>
                                }
                            })
                            .collect_view()
                    }}
                </ul>
            </div>
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use inner::ProjectSidebar;
