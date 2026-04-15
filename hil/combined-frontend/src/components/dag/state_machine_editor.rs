//! State machine block configuration editor (Leptos UI component).
//!
//! The pure config-manipulation logic lives in
//! [`crate::state_machine_config`] so it can be tested on the host.
//! This module provides only the Leptos view layer.

use block_registry::state_machine::{
    CompareOp, FieldCondition, StateMachineConfig, TopicBinding, TransitionConfig, TransitionGuard,
};
use leptos::prelude::*;
use module_traits::{FieldType, MessageField};
use wasm_bindgen::JsCast;

use crate::state_machine_config::*;

// ---------------------------------------------------------------------------
// DOM helpers
// ---------------------------------------------------------------------------

fn get_input_value(ev: &web_sys::Event) -> String {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|el| el.value())
        .unwrap_or_default()
}

fn get_select_value(ev: &web_sys::Event) -> String {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlSelectElement>().ok())
        .map(|el| el.value())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Top-level component
// ---------------------------------------------------------------------------

/// Top-level state machine editor component.
///
/// Parses the incoming `config` JSON as [`StateMachineConfig`], renders
/// an editing UI, and calls `on_config_change` with the updated JSON on
/// every edit.
#[component]
pub fn StateMachineEditor(
    config: ReadSignal<serde_json::Value>,
    on_config_change: Callback<serde_json::Value>,
) -> impl IntoView {
    let sm_config =
        Signal::derive(move || parse_state_machine_config(&config.get()).unwrap_or_default());

    let emit = move |cfg: StateMachineConfig| {
        on_config_change.run(serialize_state_machine_config(&cfg));
    };

    let (selected_transition, set_selected_transition) = signal(None::<usize>);

    view! {
        <div class="sm-editor">
            <div class="sm-section">
                <div class="sm-section-header">"States"</div>
                <div class="sm-section-body">
                    <StatesEditor
                        config=sm_config
                        on_change=Callback::new(emit)
                    />
                </div>
            </div>

            <div class="sm-section">
                <div class="sm-section-header">"Transitions"</div>
                <div class="sm-section-body">
                    <TransitionsEditor
                        config=sm_config
                        selected=selected_transition
                        set_selected=set_selected_transition
                        on_change=Callback::new(emit)
                    />
                </div>
            </div>

            {move || {
                let cfg = sm_config.get();
                let sel = selected_transition.get();
                if let Some(idx) = sel {
                    if idx < cfg.transitions.len() {
                        let transition = cfg.transitions[idx].clone();
                        let states = cfg.states.clone();
                        return view! {
                            <div class="sm-section">
                                <div class="sm-section-header">
                                    {format!("Transition #{}", idx)}
                                </div>
                                <div class="sm-section-body">
                                    <TransitionDetailEditor
                                        config=sm_config
                                        transition_index=idx
                                        transition=transition
                                        states=states
                                        on_change=Callback::new(emit)
                                    />
                                </div>
                            </div>
                        }.into_any();
                    }
                }
                view! { <div></div> }.into_any()
            }}

            <div class="sm-section">
                <div class="sm-section-header">"Input Topics"</div>
                <div class="sm-section-body">
                    <TopicBindingsEditor
                        bindings=Signal::derive(move || sm_config.get().input_topics.clone())
                        on_add=Callback::new(move |()| emit(add_input_topic(&sm_config.get_untracked())))
                        on_remove=Callback::new(move |idx: usize| emit(remove_input_topic(&sm_config.get_untracked(), idx)))
                        on_update=Callback::new(move |(idx, binding): (usize, TopicBinding)| {
                            let mut cfg = sm_config.get_untracked();
                            if let Some(b) = cfg.input_topics.get_mut(idx) {
                                *b = binding;
                            }
                            emit(cfg);
                        })
                    />
                </div>
            </div>

            <div class="sm-section">
                <div class="sm-section-header">"Output Topics"</div>
                <div class="sm-section-body">
                    <TopicBindingsEditor
                        bindings=Signal::derive(move || sm_config.get().output_topics.clone())
                        on_add=Callback::new(move |()| emit(add_output_topic(&sm_config.get_untracked())))
                        on_remove=Callback::new(move |idx: usize| emit(remove_output_topic(&sm_config.get_untracked(), idx)))
                        on_update=Callback::new(move |(idx, binding): (usize, TopicBinding)| {
                            let mut cfg = sm_config.get_untracked();
                            if let Some(b) = cfg.output_topics.get_mut(idx) {
                                *b = binding;
                            }
                            emit(cfg);
                        })
                    />
                </div>
            </div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// States editor
// ---------------------------------------------------------------------------

#[component]
fn StatesEditor(
    config: Signal<StateMachineConfig>,
    on_change: Callback<StateMachineConfig>,
) -> impl IntoView {
    view! {
        <div class="sm-states">
            {move || {
                let cfg = config.get();
                let states = cfg.states.clone();
                let initial = cfg.initial.clone();
                states.into_iter().enumerate().map(|(i, state_name)| {
                    let is_initial = state_name == initial;
                    let name_for_input = state_name.clone();
                    view! {
                        <div class="sm-state-row">
                            <input
                                type="radio"
                                name="sm-initial-state"
                                prop:checked=is_initial
                                title="Set as initial state"
                                on:change=move |_| {
                                    on_change.run(set_initial_state(&config.get_untracked(), i));
                                }
                            />
                            <input
                                type="text"
                                class="sm-input"
                                prop:value=name_for_input
                                on:change=move |ev| {
                                    let new_name = get_input_value(&ev);
                                    if !new_name.is_empty() {
                                        on_change.run(rename_state(&config.get_untracked(), i, &new_name));
                                    }
                                }
                            />
                            {if is_initial {
                                view! { <span class="sm-initial-badge">"initial"</span> }.into_any()
                            } else {
                                view! { <span></span> }.into_any()
                            }}
                            <button
                                type="button"
                                class="sm-delete-btn"
                                title="Remove state"
                                on:click=move |_| {
                                    on_change.run(remove_state(&config.get_untracked(), i));
                                }
                            >
                                "x"
                            </button>
                        </div>
                    }
                }).collect_view()
            }}
            <button
                type="button"
                class="sm-add-btn"
                on:click=move |_| {
                    on_change.run(add_state(&config.get_untracked()));
                }
            >
                "+ Add State"
            </button>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Transitions list
// ---------------------------------------------------------------------------

#[component]
fn TransitionsEditor(
    config: Signal<StateMachineConfig>,
    selected: ReadSignal<Option<usize>>,
    set_selected: WriteSignal<Option<usize>>,
    on_change: Callback<StateMachineConfig>,
) -> impl IntoView {
    view! {
        <div class="sm-transitions">
            {move || {
                let cfg = config.get();
                cfg.transitions.iter().enumerate().map(|(i, t)| {
                    let from = t.from.clone();
                    let to = t.to.clone();
                    let guard_label = guard_type_label(&t.guard).to_string();
                    let is_selected = selected.get() == Some(i);
                    view! {
                        <div
                            class=if is_selected { "sm-transition-row selected" } else { "sm-transition-row" }
                            on:click=move |_| set_selected.set(Some(i))
                        >
                            <span class="sm-transition-summary">
                                {format!("{} -> {} [{}]", from, to, guard_label)}
                            </span>
                            <button
                                type="button"
                                class="sm-delete-btn"
                                title="Remove transition"
                                on:click=move |ev| {
                                    ev.stop_propagation();
                                    let new_cfg = remove_transition(&config.get_untracked(), i);
                                    if selected.get_untracked() == Some(i) {
                                        set_selected.set(None);
                                    }
                                    on_change.run(new_cfg);
                                }
                            >
                                "x"
                            </button>
                        </div>
                    }
                }).collect_view()
            }}
            <button
                type="button"
                class="sm-add-btn"
                on:click=move |_| {
                    let new_cfg = add_transition(&config.get_untracked());
                    let new_idx = new_cfg.transitions.len().saturating_sub(1);
                    on_change.run(new_cfg);
                    set_selected.set(Some(new_idx));
                }
            >
                "+ Add Transition"
            </button>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Transition detail editor
// ---------------------------------------------------------------------------

#[component]
fn TransitionDetailEditor(
    config: Signal<StateMachineConfig>,
    transition_index: usize,
    transition: TransitionConfig,
    states: Vec<String>,
    on_change: Callback<StateMachineConfig>,
) -> impl IntoView {
    let from = transition.from.clone();
    let to = transition.to.clone();
    let guard = transition.guard.clone();
    let actions = transition.actions.clone();
    let states_for_from = states.clone();
    let states_for_to = states;

    view! {
        <div class="sm-transition-detail">
            // From state
            <div class="sm-form-row">
                <label>"From:"</label>
                <select
                    class="sm-select"
                    prop:value=from.clone()
                    on:change=move |ev| {
                        let val = get_select_value(&ev);
                        on_change.run(set_transition_from(&config.get_untracked(), transition_index, &val));
                    }
                >
                    {states_for_from.iter().map(|s| {
                        let val = s.clone();
                        let txt = s.clone();
                        let sel = *s == from;
                        view! { <option value=val selected=sel>{txt}</option> }
                    }).collect_view()}
                </select>
            </div>

            // To state
            <div class="sm-form-row">
                <label>"To:"</label>
                <select
                    class="sm-select"
                    prop:value=to.clone()
                    on:change=move |ev| {
                        let val = get_select_value(&ev);
                        on_change.run(set_transition_to(&config.get_untracked(), transition_index, &val));
                    }
                >
                    {states_for_to.iter().map(|s| {
                        let val = s.clone();
                        let txt = s.clone();
                        let sel = *s == to;
                        view! { <option value=val selected=sel>{txt}</option> }
                    }).collect_view()}
                </select>
            </div>

            // Guard type selector
            <div class="sm-form-row">
                <label>"Guard:"</label>
                <select
                    class="sm-select"
                    on:change=move |ev| {
                        let val = get_select_value(&ev);
                        let new_guard = match val.as_str() {
                            "Topic" => TransitionGuard::Topic {
                                topic: String::new(),
                                condition: None,
                            },
                            "GuardPort" => TransitionGuard::GuardPort { port: 0 },
                            _ => TransitionGuard::Unconditional,
                        };
                        on_change.run(set_transition_guard(&config.get_untracked(), transition_index, new_guard));
                    }
                >
                    <option value="Unconditional" selected=matches!(guard, TransitionGuard::Unconditional)>
                        "Unconditional"
                    </option>
                    <option value="Topic" selected=matches!(guard, TransitionGuard::Topic { .. })>
                        "Topic"
                    </option>
                    <option value="GuardPort" selected=matches!(guard, TransitionGuard::GuardPort { .. })>
                        "GuardPort"
                    </option>
                </select>
            </div>

            // Guard detail sub-editor
            {match guard.clone() {
                TransitionGuard::Topic { topic, condition } => {
                    view! {
                        <div class="sm-guard-detail">
                            <GuardTopicEditor
                                config=config
                                transition_index=transition_index
                                topic=topic
                                condition=condition
                                on_change=on_change
                            />
                        </div>
                    }.into_any()
                }
                TransitionGuard::GuardPort { port } => {
                    view! {
                        <div class="sm-guard-detail">
                            <div class="sm-form-row">
                                <label>"Port:"</label>
                                <input
                                    type="number"
                                    class="sm-input sm-input-narrow"
                                    prop:value=port.to_string()
                                    on:change=move |ev| {
                                        let val = get_input_value(&ev);
                                        if let Ok(p) = val.parse::<usize>() {
                                            on_change.run(set_transition_guard(
                                                &config.get_untracked(),
                                                transition_index,
                                                TransitionGuard::GuardPort { port: p },
                                            ));
                                        }
                                    }
                                />
                            </div>
                        </div>
                    }.into_any()
                }
                TransitionGuard::Unconditional => {
                    view! { <div></div> }.into_any()
                }
            }}

            // Actions
            <div class="sm-subsection">
                <div class="sm-subsection-header">"Actions"</div>
                {actions.iter().enumerate().map(|(ai, action)| {
                    let action_topic = action.topic.clone();
                    let action_message = action.message.clone();
                    view! {
                        <div class="sm-action-row">
                            <div class="sm-form-row">
                                <label>"Topic:"</label>
                                <input
                                    type="text"
                                    class="sm-input"
                                    prop:value=action_topic
                                    on:change=move |ev| {
                                        let val = get_input_value(&ev);
                                        let mut cfg = config.get_untracked();
                                        if let Some(t) = cfg.transitions.get_mut(transition_index) {
                                            if let Some(a) = t.actions.get_mut(ai) {
                                                a.topic = val;
                                            }
                                        }
                                        on_change.run(cfg);
                                    }
                                />
                                <button
                                    type="button"
                                    class="sm-delete-btn"
                                    title="Remove action"
                                    on:click=move |_| {
                                        on_change.run(remove_transition_action(&config.get_untracked(), transition_index, ai));
                                    }
                                >
                                    "x"
                                </button>
                            </div>
                            <div class="sm-kv-pairs">
                                {action_message.iter().enumerate().map(|(ki, (key, val))| {
                                    let k = key.clone();
                                    let v = *val;
                                    view! {
                                        <div class="sm-kv-row">
                                            <input
                                                type="text"
                                                class="sm-input sm-input-narrow"
                                                placeholder="field"
                                                prop:value=k
                                                on:change=move |ev| {
                                                    let new_key = get_input_value(&ev);
                                                    let mut cfg = config.get_untracked();
                                                    if let Some(t) = cfg.transitions.get_mut(transition_index) {
                                                        if let Some(a) = t.actions.get_mut(ai) {
                                                            if let Some(pair) = a.message.get_mut(ki) {
                                                                pair.0 = new_key;
                                                            }
                                                        }
                                                    }
                                                    on_change.run(cfg);
                                                }
                                            />
                                            <input
                                                type="number"
                                                class="sm-input sm-input-narrow"
                                                step="0.01"
                                                placeholder="value"
                                                prop:value=v.to_string()
                                                on:change=move |ev| {
                                                    let new_val_str = get_input_value(&ev);
                                                    if let Ok(new_val) = new_val_str.parse::<f64>() {
                                                        let mut cfg = config.get_untracked();
                                                        if let Some(t) = cfg.transitions.get_mut(transition_index) {
                                                            if let Some(a) = t.actions.get_mut(ai) {
                                                                if let Some(pair) = a.message.get_mut(ki) {
                                                                    pair.1 = new_val;
                                                                }
                                                            }
                                                        }
                                                        on_change.run(cfg);
                                                    }
                                                }
                                            />
                                            <button
                                                type="button"
                                                class="sm-delete-btn"
                                                title="Remove field"
                                                on:click=move |_| {
                                                    let mut cfg = config.get_untracked();
                                                    if let Some(t) = cfg.transitions.get_mut(transition_index) {
                                                        if let Some(a) = t.actions.get_mut(ai) {
                                                            if ki < a.message.len() {
                                                                a.message.remove(ki);
                                                            }
                                                        }
                                                    }
                                                    on_change.run(cfg);
                                                }
                                            >
                                                "x"
                                            </button>
                                        </div>
                                    }
                                }).collect_view()}
                                <button
                                    type="button"
                                    class="sm-add-btn sm-add-btn-small"
                                    on:click=move |_| {
                                        let mut cfg = config.get_untracked();
                                        if let Some(t) = cfg.transitions.get_mut(transition_index) {
                                            if let Some(a) = t.actions.get_mut(ai) {
                                                a.message.push((String::new(), 0.0));
                                            }
                                        }
                                        on_change.run(cfg);
                                    }
                                >
                                    "+ Add Field"
                                </button>
                            </div>
                        </div>
                    }
                }).collect_view()}
                <button
                    type="button"
                    class="sm-add-btn"
                    on:click=move |_| {
                        on_change.run(add_transition_action(&config.get_untracked(), transition_index));
                    }
                >
                    "+ Add Action"
                </button>
            </div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Topic guard editor
// ---------------------------------------------------------------------------

#[component]
fn GuardTopicEditor(
    config: Signal<StateMachineConfig>,
    transition_index: usize,
    topic: String,
    condition: Option<FieldCondition>,
    on_change: Callback<StateMachineConfig>,
) -> impl IntoView {
    let has_condition = condition.is_some();
    let cond = condition.unwrap_or(FieldCondition {
        field: String::new(),
        op: CompareOp::Eq,
        value: 0.0,
    });
    let cond_field = cond.field.clone();
    let cond_op = cond.op.clone();
    let cond_value = cond.value;

    view! {
        <div class="sm-guard-topic">
            <div class="sm-form-row">
                <label>"Topic:"</label>
                <input
                    type="text"
                    class="sm-input"
                    prop:value=topic
                    on:change=move |ev| {
                        let val = get_input_value(&ev);
                        let mut cfg = config.get_untracked();
                        if let Some(t) = cfg.transitions.get_mut(transition_index) {
                            if let TransitionGuard::Topic { ref mut topic, .. } = t.guard {
                                *topic = val;
                            }
                        }
                        on_change.run(cfg);
                    }
                />
            </div>
            <div class="sm-form-row">
                <label>
                    <input
                        type="checkbox"
                        prop:checked=has_condition
                        on:change=move |ev| {
                            let checked = ev.target()
                                .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                                .map(|el| el.checked())
                                .unwrap_or(false);
                            let mut cfg = config.get_untracked();
                            if let Some(t) = cfg.transitions.get_mut(transition_index) {
                                if let TransitionGuard::Topic { ref mut condition, .. } = t.guard {
                                    *condition = if checked {
                                        Some(FieldCondition {
                                            field: String::new(),
                                            op: CompareOp::Eq,
                                            value: 0.0,
                                        })
                                    } else {
                                        None
                                    };
                                }
                            }
                            on_change.run(cfg);
                        }
                    />
                    " Has condition"
                </label>
            </div>
            {if has_condition {
                view! {
                    <div class="sm-condition">
                        <div class="sm-form-row">
                            <label>"Field:"</label>
                            <input
                                type="text"
                                class="sm-input"
                                prop:value=cond_field
                                on:change=move |ev| {
                                    let val = get_input_value(&ev);
                                    let mut cfg = config.get_untracked();
                                    if let Some(t) = cfg.transitions.get_mut(transition_index) {
                                        if let TransitionGuard::Topic { condition: Some(ref mut c), .. } = t.guard {
                                            c.field = val;
                                        }
                                    }
                                    on_change.run(cfg);
                                }
                            />
                        </div>
                        <div class="sm-form-row">
                            <label>"Operator:"</label>
                            <select
                                class="sm-select"
                                on:change=move |ev| {
                                    let val = get_select_value(&ev);
                                    let op = compare_op_from_label(&val);
                                    let mut cfg = config.get_untracked();
                                    if let Some(t) = cfg.transitions.get_mut(transition_index) {
                                        if let TransitionGuard::Topic { condition: Some(ref mut c), .. } = t.guard {
                                            c.op = op;
                                        }
                                    }
                                    on_change.run(cfg);
                                }
                            >
                                {["==", "!=", ">", "<", ">=", "<="].iter().map(|op_label| {
                                    let is_selected = *op_label == compare_op_label(&cond_op);
                                    let val = op_label.to_string();
                                    let txt = op_label.to_string();
                                    view! { <option value=val selected=is_selected>{txt}</option> }
                                }).collect_view()}
                            </select>
                        </div>
                        <div class="sm-form-row">
                            <label>"Value:"</label>
                            <input
                                type="number"
                                class="sm-input"
                                step="0.01"
                                prop:value=cond_value.to_string()
                                on:change=move |ev| {
                                    let val_str = get_input_value(&ev);
                                    if let Ok(v) = val_str.parse::<f64>() {
                                        let mut cfg = config.get_untracked();
                                        if let Some(t) = cfg.transitions.get_mut(transition_index) {
                                            if let TransitionGuard::Topic { condition: Some(ref mut c), .. } = t.guard {
                                                c.value = v;
                                            }
                                        }
                                        on_change.run(cfg);
                                    }
                                }
                            />
                        </div>
                    </div>
                }.into_any()
            } else {
                view! { <div></div> }.into_any()
            }}
        </div>
    }
}

// ---------------------------------------------------------------------------
// Topic bindings editor
// ---------------------------------------------------------------------------

#[component]
fn TopicBindingsEditor(
    bindings: Signal<Vec<TopicBinding>>,
    on_add: Callback<()>,
    on_remove: Callback<usize>,
    on_update: Callback<(usize, TopicBinding)>,
) -> impl IntoView {
    view! {
        <div class="sm-topic-bindings">
            {move || {
                let binding_list = bindings.get();
                binding_list.into_iter().enumerate().map(|(i, binding)| {
                    let topic_name = binding.topic.clone();
                    let schema_name = binding.schema.name.clone();
                    let fields = binding.schema.fields.clone();
                    view! {
                        <div class="sm-topic-binding">
                            <div class="sm-form-row">
                                <label>"Topic:"</label>
                                <input
                                    type="text"
                                    class="sm-input"
                                    prop:value=topic_name
                                    on:change=move |ev| {
                                        let val = get_input_value(&ev);
                                        let mut b = bindings.get_untracked()[i].clone();
                                        b.topic = val;
                                        on_update.run((i, b));
                                    }
                                />
                                <button
                                    type="button"
                                    class="sm-delete-btn"
                                    title="Remove topic"
                                    on:click=move |_| on_remove.run(i)
                                >
                                    "x"
                                </button>
                            </div>
                            <div class="sm-form-row">
                                <label>"Schema:"</label>
                                <input
                                    type="text"
                                    class="sm-input"
                                    prop:value=schema_name
                                    placeholder="Schema name"
                                    on:change=move |ev| {
                                        let val = get_input_value(&ev);
                                        let mut b = bindings.get_untracked()[i].clone();
                                        b.schema.name = val;
                                        on_update.run((i, b));
                                    }
                                />
                            </div>
                            <div class="sm-schema-fields">
                                {fields.into_iter().enumerate().map(|(fi, field)| {
                                    let field_name = field.name.clone();
                                    let field_type_str = format!("{:?}", field.field_type);
                                    view! {
                                        <div class="sm-field-row">
                                            <input
                                                type="text"
                                                class="sm-input sm-input-narrow"
                                                placeholder="field name"
                                                prop:value=field_name
                                                on:change=move |ev| {
                                                    let val = get_input_value(&ev);
                                                    let mut b = bindings.get_untracked()[i].clone();
                                                    if let Some(f) = b.schema.fields.get_mut(fi) {
                                                        f.name = val;
                                                    }
                                                    on_update.run((i, b));
                                                }
                                            />
                                            <select
                                                class="sm-select"
                                                on:change=move |ev| {
                                                    let val = get_select_value(&ev);
                                                    let ft = parse_field_type(&val);
                                                    let mut b = bindings.get_untracked()[i].clone();
                                                    if let Some(f) = b.schema.fields.get_mut(fi) {
                                                        f.field_type = ft;
                                                    }
                                                    on_update.run((i, b));
                                                }
                                            >
                                                {["F32", "F64", "U8", "U16", "U32", "I32", "Bool"].iter().map(|ft| {
                                                    let is_sel = *ft == field_type_str;
                                                    let v = ft.to_string();
                                                    let t = ft.to_string();
                                                    view! { <option value=v selected=is_sel>{t}</option> }
                                                }).collect_view()}
                                            </select>
                                            <button
                                                type="button"
                                                class="sm-delete-btn"
                                                title="Remove field"
                                                on:click=move |_| {
                                                    let mut b = bindings.get_untracked()[i].clone();
                                                    if fi < b.schema.fields.len() {
                                                        b.schema.fields.remove(fi);
                                                    }
                                                    on_update.run((i, b));
                                                }
                                            >
                                                "x"
                                            </button>
                                        </div>
                                    }
                                }).collect_view()}
                                <button
                                    type="button"
                                    class="sm-add-btn sm-add-btn-small"
                                    on:click=move |_| {
                                        let mut b = bindings.get_untracked()[i].clone();
                                        b.schema.fields.push(MessageField {
                                            name: String::new(),
                                            field_type: FieldType::F64,
                                        });
                                        on_update.run((i, b));
                                    }
                                >
                                    "+ Add Field"
                                </button>
                            </div>
                        </div>
                    }
                }).collect_view()
            }}
            <button
                type="button"
                class="sm-add-btn"
                on:click=move |_| on_add.run(())
            >
                "+ Add Topic"
            </button>
        </div>
    }
}

fn parse_field_type(s: &str) -> FieldType {
    match s {
        "F32" => FieldType::F32,
        "F64" => FieldType::F64,
        "U8" => FieldType::U8,
        "U16" => FieldType::U16,
        "U32" => FieldType::U32,
        "I32" => FieldType::I32,
        "Bool" => FieldType::Bool,
        _ => FieldType::F64,
    }
}
