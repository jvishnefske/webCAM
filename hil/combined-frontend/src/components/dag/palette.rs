//! Block palette with categorized sub-menus.

use leptos::prelude::*;

use configurable_blocks::registry::{descriptors_by_category, BlockDescriptor};
use configurable_blocks::schema::BlockCategory;

/// Palette showing block categories as collapsible sub-menus.
#[component]
pub fn BlockPalette(on_add: Callback<String>) -> impl IntoView {
    let groups = descriptors_by_category();

    view! {
        <div class="dag-palette">
            <div class="palette-title">"Block Palette"</div>
            {groups.into_iter().map(|(category, entries)| {
                view! {
                    <PaletteCategory category=category entries=entries on_add=on_add />
                }
            }).collect_view()}
        </div>
    }
}

/// A single collapsible category in the palette.
#[component]
fn PaletteCategory(
    category: BlockCategory,
    entries: Vec<BlockDescriptor>,
    on_add: Callback<String>,
) -> impl IntoView {
    let (expanded, set_expanded) = signal(true);
    let label = category.label().to_string();

    view! {
        <div class="palette-category">
            <button
                class="palette-category-header"
                on:click=move |_| set_expanded.set(!expanded.get())
            >
                <span class="palette-chevron">
                    {move || if expanded.get() { "\u{25BE}" } else { "\u{25B8}" }}
                </span>
                {label}
            </button>
            <div
                class="palette-category-body"
                style=move || if expanded.get() { "display:block" } else { "display:none" }
            >
                {entries.into_iter().map(|entry| {
                    let block_type = entry.block_type.clone();
                    let display_name = entry.display_name.clone();
                    let description = entry.description.clone();
                    view! {
                        <button
                            class="palette-block-btn"
                            title=description
                            on:click=move |_| on_add.run(block_type.clone())
                        >
                            {display_name}
                        </button>
                    }
                }).collect_view()}
            </div>
        </div>
    }
}
