//! Project sidebar: save, load, delete projects from localStorage.

use leptos::prelude::*;

use super::storage;

/// Sidebar component showing saved projects with load/delete buttons.
#[component]
pub fn ProjectSidebar(
    /// Current project name signal.
    project_name: ReadSignal<String>,
    /// Setter for the project name.
    set_project_name: WriteSignal<String>,
    /// Callback to save the current project.
    on_save: Callback<()>,
    /// Callback to load a project by name.
    on_load: Callback<String>,
    /// Callback to create a new (empty) project.
    on_new: Callback<()>,
) -> impl IntoView {
    // List of project names from localStorage.
    let (project_list, set_project_list) = signal(Vec::<String>::new());
    let (sidebar_status, set_sidebar_status) = signal(String::new());

    // Refresh the project list.
    let refresh_list = move || match storage::list_projects() {
        Ok(names) => set_project_list.set(names),
        Err(e) => set_sidebar_status.set(format!("Error: {e}")),
    };

    // Initial load.
    refresh_list();

    let on_save_click = move |_| {
        on_save.run(());
        refresh_list();
        set_sidebar_status.set("Saved.".into());
    };

    let on_new_click = move |_| {
        on_new.run(());
        set_sidebar_status.set(String::new());
    };

    view! {
        <div class="dag-project-bar">
            <input
                type="text"
                class="dag-project-name-input"
                placeholder="Project name"
                prop:value=move || project_name.get()
                on:input=move |ev| {
                    use wasm_bindgen::JsCast;
                    if let Some(input) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) {
                        set_project_name.set(input.value());
                    }
                }
            />
            <div class="dag-project-actions">
                <button class="btn btn-primary btn-sm" on:click=on_save_click>"Save"</button>
                <button class="btn btn-secondary btn-sm" on:click=on_new_click>"New"</button>
            </div>

            // Status message
            {move || {
                let status = sidebar_status.get();
                if status.is_empty() {
                    None
                } else {
                    Some(view! { <div class="sidebar-status">{status}</div> })
                }
            }}

            // Saved project list
            <div class="sidebar-project-list">
                <For
                    each=move || project_list.get()
                    key=|name| name.clone()
                    let:name
                >
                    {
                        let name_for_load = name.clone();
                        let name_for_delete = name.clone();
                        let name_display = name.clone();
                        let is_current = {
                            let n = name.clone();
                            move || project_name.get() == n
                        };
                        view! {
                            <div class=move || if is_current() { "sidebar-project-item active" } else { "sidebar-project-item" }>
                                <span class="sidebar-project-name">{name_display}</span>
                                <div class="sidebar-project-btns">
                                    <button
                                        class="btn btn-sm btn-secondary"
                                        on:click={
                                            let name = name_for_load.clone();
                                            let refresh = refresh_list;
                                            move |_| {
                                                on_load.run(name.clone());
                                                refresh();
                                            }
                                        }
                                    >"Load"</button>
                                    <button
                                        class="btn btn-sm btn-danger"
                                        on:click={
                                            let name = name_for_delete.clone();
                                            let refresh = refresh_list;
                                            let set_status = set_sidebar_status;
                                            move |_| {
                                                let _ = storage::delete_project(&name);
                                                refresh();
                                                set_status.set("Deleted.".into());
                                            }
                                        }
                                    >"Del"</button>
                                </div>
                            </div>
                        }
                    }
                </For>
            </div>
        </div>
    }
}
