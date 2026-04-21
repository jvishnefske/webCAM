//! CAM panel Leptos component.
//!
//! Provides a complete CAM workflow: file upload (STL/SVG), machine and
//! strategy configuration, G-code generation via `rustcam`, and output
//! with copy/download actions.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{DragEvent, FileReader, HtmlInputElement};

use crate::cam_config::{build_cam_config, default_cnc_params, default_laser_params, CamParams};

// ---------------------------------------------------------------------------
// File data enum — STL bytes vs SVG text
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum FileData {
    Stl(Vec<u8>),
    Svg(String),
}

// ---------------------------------------------------------------------------
// Strategy lists
// ---------------------------------------------------------------------------

const CNC_STRATEGIES: &[(&str, &str)] = &[
    ("contour", "Contour"),
    ("pocket", "Pocket"),
    ("slice", "Slice"),
    ("surface3d", "Surface 3D"),
    ("perimeter", "Perimeter"),
];

const SURFACE_PATTERNS: &[(&str, &str)] = &[
    ("zigzag", "Zig-zag"),
    ("one_way", "One-way"),
    ("spiral", "Spiral"),
];

const LASER_STRATEGIES: &[(&str, &str)] = &[
    ("laser_cut", "Cut"),
    ("laser_engrave", "Engrave"),
    ("contour", "Contour"),
    ("pocket", "Pocket"),
    ("perimeter", "Perimeter"),
];

const TOOL_TYPES: &[(&str, &str)] = &[
    ("end_mill", "End Mill"),
    ("ball_end", "Ball End"),
    ("face_mill", "Face Mill"),
];

// ---------------------------------------------------------------------------
// CamPanel component
// ---------------------------------------------------------------------------

#[component]
pub fn CamPanel() -> impl IntoView {
    // -- File state --
    let (file_data, set_file_data) = signal(None::<FileData>);
    let (file_name, set_file_name) = signal(String::new());

    // -- Machine / strategy --
    let (machine_type, set_machine_type) = signal("cnc_mill".to_string());
    let (strategy, set_strategy) = signal("contour".to_string());
    let is_laser = Signal::derive(move || machine_type.get() == "laser_cutter");

    // -- Parameters --
    let (params, set_params) = signal(default_cnc_params());

    // -- Output --
    let (gcode_output, set_gcode_output) = signal(String::new());
    let (error_msg, set_error_msg) = signal(String::new());
    let (status_msg, set_status_msg) = signal(String::new());
    let (copy_label, set_copy_label) = signal("Copy".to_string());

    // Line count derived from output.
    let line_count = Signal::derive(move || {
        let g = gcode_output.get();
        if g.is_empty() {
            0
        } else {
            g.lines().count()
        }
    });

    // -- File input ref --
    let file_input_ref = NodeRef::<leptos::html::Input>::new();

    // -- Machine type change resets strategy --
    Effect::new(move |_| {
        let mt = machine_type.get();
        let default_strat = if mt == "laser_cutter" {
            "laser_cut"
        } else {
            "contour"
        };
        set_strategy.set(default_strat.to_string());

        // Reset params to defaults for the selected machine.
        if mt == "laser_cutter" {
            set_params.set(default_laser_params());
        } else {
            set_params.set(default_cnc_params());
        }
    });

    // -- File handling closures --
    let handle_file_load = move |name: String, data: FileData| {
        set_file_name.set(name);
        set_file_data.set(Some(data));
        set_error_msg.set(String::new());
        set_status_msg.set(String::new());
    };

    let read_file = move |file: web_sys::File| {
        let name = file.name();
        let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();

        match ext.as_str() {
            "stl" => {
                let reader = FileReader::new().expect("FileReader");
                let reader_clone = reader.clone();
                let onload = Closure::wrap(Box::new(move |_: web_sys::ProgressEvent| {
                    if let Ok(result) = reader_clone.result() {
                        let array_buffer = result
                            .dyn_into::<js_sys::ArrayBuffer>()
                            .expect("ArrayBuffer");
                        let bytes = js_sys::Uint8Array::new(&array_buffer).to_vec();
                        handle_file_load(name.clone(), FileData::Stl(bytes));
                    }
                }) as Box<dyn FnMut(_)>);
                reader.set_onload(Some(onload.as_ref().unchecked_ref()));
                onload.forget();
                let _ = reader.read_as_array_buffer(&file);
            }
            "svg" => {
                let reader = FileReader::new().expect("FileReader");
                let reader_clone = reader.clone();
                let onload = Closure::wrap(Box::new(move |_: web_sys::ProgressEvent| {
                    if let Ok(result) = reader_clone.result() {
                        if let Some(text) = result.as_string() {
                            handle_file_load(name.clone(), FileData::Svg(text));
                        }
                    }
                }) as Box<dyn FnMut(_)>);
                reader.set_onload(Some(onload.as_ref().unchecked_ref()));
                onload.forget();
                let _ = reader.read_as_text(&file);
            }
            other => {
                set_error_msg.set(format!("Unsupported file type: .{other}"));
            }
        }
    };

    let on_file_input = move |ev: leptos::ev::Event| {
        let target: HtmlInputElement = ev.target().unwrap().unchecked_into();
        if let Some(files) = target.files() {
            if let Some(file) = files.get(0) {
                read_file(file);
            }
        }
    };

    let on_drop = move |ev: DragEvent| {
        ev.prevent_default();
        if let Some(dt) = ev.data_transfer() {
            if let Some(files) = dt.files() {
                if let Some(file) = files.get(0) {
                    read_file(file);
                }
            }
        }
    };

    let on_dragover = move |ev: DragEvent| {
        ev.prevent_default();
    };

    // -- Generate G-code --
    let on_generate = move |_| {
        set_error_msg.set(String::new());
        set_status_msg.set(String::new());

        let data = match file_data.get() {
            Some(d) => d,
            None => {
                set_error_msg.set("No file loaded. Please upload an STL or SVG file.".into());
                return;
            }
        };

        let mt = machine_type.get();
        let strat = strategy.get();
        let p = params.get();
        let config_json = build_cam_config(&mt, &strat, &p);
        let config_str = serde_json::to_string(&config_json).unwrap_or_default();

        let result = match &data {
            FileData::Stl(bytes) => rustcam::process_stl_impl(bytes, &config_str),
            FileData::Svg(text) => rustcam::process_svg_impl(text, &config_str),
        };

        match result {
            Ok(gcode) => {
                let lines = gcode.lines().count();
                set_status_msg.set(format!("Done -- {lines} lines of G-code."));
                set_gcode_output.set(gcode);
            }
            Err(e) => {
                set_error_msg.set(e);
            }
        }
    };

    // -- Copy to clipboard --
    let on_copy = move |_| {
        let text = gcode_output.get();
        if text.is_empty() {
            return;
        }
        let window = web_sys::window().expect("window");
        let navigator = window.navigator();
        let clipboard = navigator.clipboard();
        let promise = clipboard.write_text(&text);
        let set_label = set_copy_label;
        let cb = Closure::wrap(Box::new(move |_: JsValue| {
            set_label.set("Copied!".to_string());
            let reset = set_label;
            gloo_timers::callback::Timeout::new(1500, move || {
                reset.set("Copy".to_string());
            })
            .forget();
        }) as Box<dyn FnMut(_)>);
        let _ = promise.then(&cb);
        cb.forget();
    };

    // -- Download G-code --
    let on_download = move |_| {
        let text = gcode_output.get();
        if text.is_empty() {
            return;
        }
        let window = web_sys::window().expect("window");
        let document = window.document().expect("document");

        // Create a Blob from the text.
        let parts = js_sys::Array::new();
        parts.push(&JsValue::from_str(&text));
        let blob = web_sys::Blob::new_with_str_sequence(&parts).expect("Blob");

        let url = web_sys::Url::create_object_url_with_blob(&blob).expect("URL");
        let a: web_sys::HtmlAnchorElement = document
            .create_element("a")
            .expect("a element")
            .unchecked_into();
        a.set_href(&url);
        let download_name = {
            let fname = file_name.get();
            if fname.is_empty() {
                "output.gcode".to_string()
            } else {
                // Replace the extension.
                match fname.rsplit_once('.') {
                    Some((stem, _)) => format!("{stem}.gcode"),
                    None => format!("{fname}.gcode"),
                }
            }
        };
        a.set_download(&download_name);
        a.click();
        let _ = web_sys::Url::revoke_object_url(&url);
    };

    // -- Helper: update a single field in params --
    let update_param = move |updater: Box<dyn Fn(&mut CamParams)>| {
        set_params.update(move |p| updater(p));
    };

    // -- Render --
    view! {
        <div class="cam-panel" style="padding: 1rem; display: flex; flex-direction: column; gap: 1rem;">

            // -- File upload area --
            <div
                class="drop-zone"
                style="border: 2px dashed #555; border-radius: 8px; padding: 2rem; text-align: center; cursor: pointer;"
                on:click=move |_| {
                    if let Some(input) = file_input_ref.get() {
                        let el: &HtmlInputElement = &input;
                        el.click();
                    }
                }
                on:drop=on_drop
                on:dragover=on_dragover
            >
                <p style="margin: 0;">{move || {
                    let name = file_name.get();
                    if name.is_empty() {
                        "Drop STL or SVG file here, or click to browse".to_string()
                    } else {
                        name
                    }
                }}</p>
                <input
                    node_ref=file_input_ref
                    type="file"
                    accept=".stl,.svg"
                    style="display: none;"
                    on:change=on_file_input
                />
            </div>

            // -- Configuration section --
            <div style="display: flex; flex-wrap: wrap; gap: 1rem;">

                // -- Machine type --
                <fieldset style="border: 1px solid #555; border-radius: 4px; padding: 0.5rem; min-width: 180px;">
                    <legend>"Machine Type"</legend>
                    <label style="display: block; margin-bottom: 4px;">
                        <input
                            type="radio"
                            name="machine_type"
                            value="cnc_mill"
                            checked=move || !is_laser.get()
                            on:change=move |_| set_machine_type.set("cnc_mill".into())
                        />
                        " CNC Mill"
                    </label>
                    <label style="display: block;">
                        <input
                            type="radio"
                            name="machine_type"
                            value="laser_cutter"
                            checked=move || is_laser.get()
                            on:change=move |_| set_machine_type.set("laser_cutter".into())
                        />
                        " Laser Cutter"
                    </label>
                </fieldset>

                // -- Strategy --
                <fieldset style="border: 1px solid #555; border-radius: 4px; padding: 0.5rem; min-width: 180px;">
                    <legend>"Strategy"</legend>
                    <select
                        style="width: 100%; padding: 4px;"
                        on:change=move |ev| {
                            let val = event_target_value(&ev);
                            set_strategy.set(val);
                        }
                        prop:value=move || strategy.get()
                    >
                        {move || {
                            let strategies = if is_laser.get() { LASER_STRATEGIES } else { CNC_STRATEGIES };
                            strategies.iter().map(|(value, label)| {
                                let selected = strategy.get() == *value;
                                view! {
                                    <option value=*value selected=selected>{*label}</option>
                                }
                            }).collect_view()
                        }}
                    </select>
                </fieldset>

                // -- Pattern (Surface 3D only) --
                <Show when=move || strategy.get() == "surface3d">
                    <fieldset style="border: 1px solid #555; border-radius: 4px; padding: 0.5rem; min-width: 160px;">
                        <legend>"Pattern"</legend>
                        <select
                            style="width: 100%; padding: 4px;"
                            on:change=move |ev| {
                                let val = event_target_value(&ev);
                                update_param(Box::new(move |p: &mut CamParams| p.pattern = val.clone()));
                            }
                            prop:value=move || params.get().pattern
                        >
                            {SURFACE_PATTERNS.iter().map(|(value, label)| {
                                view! { <option value=*value>{*label}</option> }
                            }).collect_view()}
                        </select>
                    </fieldset>
                </Show>

                // -- Tool config (CNC only) --
                <Show when=move || !is_laser.get()>
                    <fieldset style="border: 1px solid #555; border-radius: 4px; padding: 0.5rem; min-width: 220px;">
                        <legend>"Tool"</legend>
                        <label style="display: block; margin-bottom: 4px;">
                            "Type: "
                            <select
                                style="padding: 4px;"
                                on:change=move |ev| {
                                    let val = event_target_value(&ev);
                                    update_param(Box::new(move |p: &mut CamParams| {
                                        p.tool_type = val.clone();
                                        if p.tool_type == "ball_end" {
                                            p.corner_radius = p.tool_diameter / 2.0;
                                        }
                                    }));
                                }
                            >
                                {TOOL_TYPES.iter().map(|(value, label)| {
                                    view! { <option value=*value>{*label}</option> }
                                }).collect_view()}
                            </select>
                        </label>
                        <label style="display: block; margin-bottom: 4px;">
                            "Diameter (mm): "
                            <input
                                type="number"
                                step="0.1"
                                min="0.1"
                                style="width: 80px; padding: 2px;"
                                prop:value=move || params.get().tool_diameter.to_string()
                                on:change=move |ev| {
                                    let val: f64 = event_target_value(&ev).parse().unwrap_or(3.175);
                                    update_param(Box::new(move |p: &mut CamParams| p.tool_diameter = val));
                                }
                            />
                        </label>
                        <Show when=move || params.get().tool_type == "ball_end">
                            <label style="display: block; margin-bottom: 4px;">
                                "Corner Radius (mm): "
                                <input
                                    type="number"
                                    step="0.01"
                                    min="0"
                                    style="width: 80px; padding: 2px;"
                                    prop:value=move || params.get().corner_radius.to_string()
                                    on:change=move |ev| {
                                        let val: f64 = event_target_value(&ev).parse().unwrap_or(0.0);
                                        update_param(Box::new(move |p: &mut CamParams| p.corner_radius = val));
                                    }
                                />
                            </label>
                        </Show>
                        <label style="display: block; margin-bottom: 4px;">
                            "Step-over (mm): "
                            <input
                                type="number"
                                step="0.1"
                                min="0.1"
                                style="width: 80px; padding: 2px;"
                                prop:value=move || params.get().step_over.to_string()
                                on:change=move |ev| {
                                    let val: f64 = event_target_value(&ev).parse().unwrap_or(1.5);
                                    update_param(Box::new(move |p: &mut CamParams| p.step_over = val));
                                }
                            />
                        </label>
                    </fieldset>
                </Show>
            </div>

            // -- CNC parameters --
            <Show when=move || !is_laser.get()>
                <fieldset style="border: 1px solid #555; border-radius: 4px; padding: 0.5rem;">
                    <legend>"CNC Parameters"</legend>
                    <div style="display: flex; flex-wrap: wrap; gap: 0.75rem;">
                        <label>
                            "Cut Depth (mm): "
                            <input
                                type="number"
                                step="0.1"
                                style="width: 80px; padding: 2px;"
                                prop:value=move || params.get().cut_depth.to_string()
                                on:change=move |ev| {
                                    let val: f64 = event_target_value(&ev).parse().unwrap_or(-1.0);
                                    update_param(Box::new(move |p: &mut CamParams| p.cut_depth = val));
                                }
                            />
                        </label>
                        <label>
                            "Step-down (mm): "
                            <input
                                type="number"
                                step="0.1"
                                min="0.1"
                                style="width: 80px; padding: 2px;"
                                prop:value=move || params.get().step_down.to_string()
                                on:change=move |ev| {
                                    let val: f64 = event_target_value(&ev).parse().unwrap_or(1.0);
                                    update_param(Box::new(move |p: &mut CamParams| p.step_down = val));
                                }
                            />
                        </label>
                        <label>
                            "Feed Rate (mm/min): "
                            <input
                                type="number"
                                step="10"
                                min="1"
                                style="width: 80px; padding: 2px;"
                                prop:value=move || params.get().feed_rate.to_string()
                                on:change=move |ev| {
                                    let val: f64 = event_target_value(&ev).parse().unwrap_or(800.0);
                                    update_param(Box::new(move |p: &mut CamParams| p.feed_rate = val));
                                }
                            />
                        </label>
                        <label>
                            "Plunge Rate (mm/min): "
                            <input
                                type="number"
                                step="10"
                                min="1"
                                style="width: 80px; padding: 2px;"
                                prop:value=move || params.get().plunge_rate.to_string()
                                on:change=move |ev| {
                                    let val: f64 = event_target_value(&ev).parse().unwrap_or(300.0);
                                    update_param(Box::new(move |p: &mut CamParams| p.plunge_rate = val));
                                }
                            />
                        </label>
                        <label>
                            "Spindle Speed (RPM): "
                            <input
                                type="number"
                                step="100"
                                min="0"
                                style="width: 80px; padding: 2px;"
                                prop:value=move || params.get().spindle_speed.to_string()
                                on:change=move |ev| {
                                    let val: f64 = event_target_value(&ev).parse().unwrap_or(12000.0);
                                    update_param(Box::new(move |p: &mut CamParams| p.spindle_speed = val));
                                }
                            />
                        </label>
                        <label>
                            "Safe Z (mm): "
                            <input
                                type="number"
                                step="0.5"
                                min="0"
                                style="width: 80px; padding: 2px;"
                                prop:value=move || params.get().safe_z.to_string()
                                on:change=move |ev| {
                                    let val: f64 = event_target_value(&ev).parse().unwrap_or(5.0);
                                    update_param(Box::new(move |p: &mut CamParams| p.safe_z = val));
                                }
                            />
                        </label>
                    </div>
                </fieldset>
            </Show>

            // -- Laser parameters --
            <Show when=move || is_laser.get()>
                <fieldset style="border: 1px solid #555; border-radius: 4px; padding: 0.5rem;">
                    <legend>"Laser Parameters"</legend>
                    <div style="display: flex; flex-wrap: wrap; gap: 0.75rem;">
                        <label>
                            "Laser Power (%): "
                            <input
                                type="number"
                                step="1"
                                min="0"
                                max="100"
                                style="width: 80px; padding: 2px;"
                                prop:value=move || params.get().laser_power.to_string()
                                on:change=move |ev| {
                                    let val: f64 = event_target_value(&ev).parse().unwrap_or(100.0);
                                    update_param(Box::new(move |p: &mut CamParams| p.laser_power = val));
                                }
                            />
                        </label>
                        <label>
                            "Feed Rate (mm/min): "
                            <input
                                type="number"
                                step="10"
                                min="1"
                                style="width: 80px; padding: 2px;"
                                prop:value=move || params.get().feed_rate.to_string()
                                on:change=move |ev| {
                                    let val: f64 = event_target_value(&ev).parse().unwrap_or(1000.0);
                                    update_param(Box::new(move |p: &mut CamParams| p.feed_rate = val));
                                }
                            />
                        </label>
                        <label>
                            "Passes: "
                            <input
                                type="number"
                                step="1"
                                min="1"
                                style="width: 60px; padding: 2px;"
                                prop:value=move || params.get().laser_passes.to_string()
                                on:change=move |ev| {
                                    let val: u32 = event_target_value(&ev).parse().unwrap_or(1);
                                    update_param(Box::new(move |p: &mut CamParams| p.laser_passes = val));
                                }
                            />
                        </label>
                        <label style="display: flex; align-items: center; gap: 4px;">
                            <input
                                type="checkbox"
                                prop:checked=move || params.get().air_assist
                                on:change=move |ev| {
                                    let checked = event_target_checked(&ev);
                                    update_param(Box::new(move |p: &mut CamParams| p.air_assist = checked));
                                }
                            />
                            " Air Assist"
                        </label>
                    </div>
                </fieldset>
            </Show>

            // -- Generate button --
            <div style="display: flex; align-items: center; gap: 1rem;">
                <button
                    style="padding: 0.5rem 1.5rem; font-weight: bold;"
                    disabled=move || file_data.get().is_none()
                    on:click=on_generate
                >
                    "Generate G-code"
                </button>
                <span style="font-size: 0.85rem; opacity: 0.8;">{move || status_msg.get()}</span>
            </div>

            // -- Error display --
            <Show when=move || !error_msg.get().is_empty()>
                <div style="color: #e74c3c; padding: 0.5rem; border: 1px solid #e74c3c; border-radius: 4px;">
                    {move || error_msg.get()}
                </div>
            </Show>

            // -- G-code output --
            <Show when=move || !gcode_output.get().is_empty()>
                <div style="display: flex; flex-direction: column; gap: 0.5rem;">
                    <div style="display: flex; justify-content: space-between; align-items: center;">
                        <span style="font-size: 0.85rem; opacity: 0.8;">
                            {move || format!("{} lines", line_count.get())}
                        </span>
                        <div style="display: flex; gap: 0.5rem;">
                            <button
                                style="padding: 0.25rem 0.75rem;"
                                on:click=on_copy
                            >
                                {move || copy_label.get()}
                            </button>
                            <button
                                style="padding: 0.25rem 0.75rem;"
                                on:click=on_download
                            >
                                "Download"
                            </button>
                        </div>
                    </div>
                    <textarea
                        readonly=true
                        rows="20"
                        style="width: 100%; font-family: monospace; font-size: 0.8rem; resize: vertical;"
                        prop:value=move || gcode_output.get()
                    />
                </div>
            </Show>
        </div>
    }
}

/// Extract the value from an event target as a string.
fn event_target_value(ev: &leptos::ev::Event) -> String {
    let target: HtmlInputElement = ev.target().unwrap().unchecked_into();
    target.value()
}

/// Extract the checked state from a checkbox event target.
fn event_target_checked(ev: &leptos::ev::Event) -> bool {
    let target: HtmlInputElement = ev.target().unwrap().unchecked_into();
    target.checked()
}
