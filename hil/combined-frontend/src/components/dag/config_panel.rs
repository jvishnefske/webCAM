//! Block configuration editor panel.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use configurable_blocks::schema::{ConfigField, FieldKind};

fn get_input_value(ev: &web_sys::Event) -> String {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|el| el.value())
        .unwrap_or_default()
}

fn get_input_checked(ev: &web_sys::Event) -> bool {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|el| el.checked())
        .unwrap_or(false)
}

/// Panel for editing a selected block's configuration.
#[component]
pub fn ConfigPanel(
    block_type: Signal<Option<String>>,
    config_fields: Signal<Vec<ConfigField>>,
    config_values: Signal<serde_json::Value>,
    on_change: Callback<(String, serde_json::Value)>,
    channels_text: Signal<String>,
    il_text: Signal<String>,
) -> impl IntoView {
    view! {
        <div class="dag-config-panel">
            {move || {
                if let Some(bt) = block_type.get() {
                    view! {
                        <div>
                            <div class="config-panel-title">{bt}</div>
                            <div class="config-fields">
                                <For
                                    each=move || config_fields.get()
                                    key=|f| f.key.clone()
                                    let:field
                                >
                                    <ConfigFieldEditor
                                        field=field.clone()
                                        value=Signal::derive(move || {
                                            config_values.get()
                                                .get(&field.key)
                                                .cloned()
                                                .unwrap_or(field.default.clone())
                                        })
                                        on_change=on_change
                                    />
                                </For>
                            </div>
                            <div class="config-section-title">"Channels"</div>
                            <pre class="config-channels">{move || channels_text.get()}</pre>
                            <div class="config-section-title">"Generated IL"</div>
                            <pre class="config-il">{move || il_text.get()}</pre>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="config-empty">
                            "Select a block from the palette or canvas to configure it."
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}

/// Editor for a single config field.
#[component]
fn ConfigFieldEditor(
    field: ConfigField,
    value: Signal<serde_json::Value>,
    on_change: Callback<(String, serde_json::Value)>,
) -> impl IntoView {
    let label = field.label.clone();

    match field.kind {
        FieldKind::Float => {
            let key = field.key.clone();
            view! {
                <div class="form-group">
                    <label>{label}</label>
                    <input
                        type="number"
                        step="0.01"
                        prop:value=move || {
                            value.get().as_f64().map(|v| v.to_string()).unwrap_or_default()
                        }
                        on:change=move |ev| {
                            let val_str = get_input_value(&ev);
                            if let Ok(v) = val_str.parse::<f64>() {
                                on_change.run((key.clone(), serde_json::json!(v)));
                            }
                        }
                    />
                </div>
            }
            .into_any()
        }
        FieldKind::Int => {
            let key = field.key.clone();
            view! {
                <div class="form-group">
                    <label>{label}</label>
                    <input
                        type="number"
                        step="1"
                        prop:value=move || {
                            value.get().as_i64().map(|v| v.to_string()).unwrap_or_default()
                        }
                        on:change=move |ev| {
                            let val_str = get_input_value(&ev);
                            if let Ok(v) = val_str.parse::<i64>() {
                                on_change.run((key.clone(), serde_json::json!(v)));
                            }
                        }
                    />
                </div>
            }
            .into_any()
        }
        FieldKind::Text => {
            let key = field.key.clone();
            view! {
                <div class="form-group">
                    <label>{label}</label>
                    <input
                        type="text"
                        prop:value=move || {
                            value.get().as_str().unwrap_or_default().to_string()
                        }
                        on:change=move |ev| {
                            let val_str = get_input_value(&ev);
                            on_change.run((key.clone(), serde_json::json!(val_str)));
                        }
                    />
                </div>
            }
            .into_any()
        }
        FieldKind::Bool => {
            let key = field.key.clone();
            view! {
                <div class="form-group form-group-checkbox">
                    <label>
                        <input
                            type="checkbox"
                            prop:checked=move || {
                                value.get().as_bool().unwrap_or(false)
                            }
                            on:change=move |ev| {
                                let checked = get_input_checked(&ev);
                                on_change.run((key.clone(), serde_json::json!(checked)));
                            }
                        />
                        {label}
                    </label>
                </div>
            }
            .into_any()
        }
        FieldKind::Select(options) => {
            let key = field.key.clone();
            view! {
                <div class="form-group">
                    <label>{label}</label>
                    <select
                        prop:value=move || {
                            value.get().as_str().unwrap_or_default().to_string()
                        }
                        on:change=move |ev| {
                            let val_str = get_input_value(&ev);
                            on_change.run((key.clone(), serde_json::json!(val_str)));
                        }
                    >
                        {options.iter().map(|opt| {
                            let v = opt.clone();
                            let t = opt.clone();
                            view! { <option value=v>{t}</option> }
                        }).collect_view()}
                    </select>
                </div>
            }
            .into_any()
        }
    }
}
