//! Panel model types: widgets, bindings, and the panel document.
//!
//! These types are **not** gated behind `target_arch = "wasm32"` so that
//! they can be tested with `cargo test` on the host.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// WidgetKind
// ---------------------------------------------------------------------------

/// The kind of widget and its kind-specific parameters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WidgetKind {
    Toggle,
    Slider { min: f64, max: f64, step: f64 },
    Gauge { min: f64, max: f64 },
    Label,
    Button,
    Indicator,
}

impl WidgetKind {
    /// Human-readable display name for palette buttons.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Toggle => "Toggle",
            Self::Slider { .. } => "Slider",
            Self::Gauge { .. } => "Gauge",
            Self::Label => "Label",
            Self::Button => "Button",
            Self::Indicator => "Indicator",
        }
    }

    /// Fixed set of binding roles for this kind.
    ///
    /// The number and direction of bindings is a property of the widget, not a
    /// user-editable list: a Slider emits exactly one output; a Gauge reads one
    /// input. The user chooses the topic (for Input) or accepts/edits the
    /// auto-generated topic name (for Output), but cannot add or remove roles.
    pub fn binding_schema(&self) -> &'static [BindingRole] {
        match self {
            Self::Toggle | Self::Slider { .. } | Self::Button => &[BindingRole {
                name: "value",
                direction: BindingDirection::Output,
            }],
            Self::Gauge { .. } | Self::Label | Self::Indicator => &[BindingRole {
                name: "value",
                direction: BindingDirection::Input,
            }],
        }
    }
}

/// A slot in a widget's binding schema.
#[derive(Debug, Clone, Copy)]
pub struct BindingRole {
    pub name: &'static str,
    pub direction: BindingDirection,
}

// ---------------------------------------------------------------------------
// BindingDirection
// ---------------------------------------------------------------------------

/// Whether the binding reads from or writes to a topic.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BindingDirection {
    Input,
    Output,
}

// ---------------------------------------------------------------------------
// ChannelBinding
// ---------------------------------------------------------------------------

/// Connects a widget to a pub/sub topic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelBinding {
    pub direction: BindingDirection,
    pub topic: String,
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// A single widget placed on the panel canvas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Widget {
    pub id: u32,
    pub kind: WidgetKind,
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub bindings: Vec<ChannelBinding>,
}

// ---------------------------------------------------------------------------
// PanelModel
// ---------------------------------------------------------------------------

/// Root document model for a panel layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelModel {
    pub name: String,
    pub widgets: Vec<Widget>,
    next_id: u32,
}

impl PanelModel {
    /// Create an empty panel with the given name.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            widgets: Vec::new(),
            next_id: 1,
        }
    }

    /// Add a widget of the given kind and label with sensible default size.
    /// Returns the new widget's unique id.
    pub fn add_widget(&mut self, kind: WidgetKind, label: &str) -> u32 {
        let id = self.next_id;
        self.next_id += 1;

        let (width, height) = default_size(&kind);
        let bindings = kind
            .binding_schema()
            .iter()
            .map(|role| ChannelBinding {
                direction: role.direction,
                topic: default_topic(&kind, id, role),
            })
            .collect();

        self.widgets.push(Widget {
            id,
            kind,
            label: label.to_string(),
            x: 20.0,
            y: 20.0,
            width,
            height,
            bindings,
        });

        id
    }

    /// Remove the widget with `id`. Returns `true` if it was found.
    pub fn remove_widget(&mut self, id: u32) -> bool {
        let before = self.widgets.len();
        self.widgets.retain(|w| w.id != id);
        self.widgets.len() < before
    }

    /// Immutable lookup by id.
    pub fn get_widget(&self, id: u32) -> Option<&Widget> {
        self.widgets.iter().find(|w| w.id == id)
    }

    /// Mutable lookup by id.
    pub fn get_widget_mut(&mut self, id: u32) -> Option<&mut Widget> {
        self.widgets.iter_mut().find(|w| w.id == id)
    }
}

/// Auto-generated default topic for a widget's Output binding, or empty string
/// for Input bindings (user must pick a topic to read from).
fn default_topic(kind: &WidgetKind, id: u32, role: &BindingRole) -> String {
    match role.direction {
        BindingDirection::Input => String::new(),
        BindingDirection::Output => {
            let slug = match kind {
                WidgetKind::Slider { .. } => "slider",
                WidgetKind::Toggle => "toggle",
                WidgetKind::Button => "button",
                WidgetKind::Gauge { .. } => "gauge",
                WidgetKind::Label => "label",
                WidgetKind::Indicator => "indicator",
            };
            format!("panel/{slug}_{id}")
        }
    }
}

/// Sensible default size for each widget kind.
fn default_size(kind: &WidgetKind) -> (f64, f64) {
    match kind {
        WidgetKind::Toggle => (100.0, 40.0),
        WidgetKind::Slider { .. } => (200.0, 40.0),
        WidgetKind::Gauge { .. } => (120.0, 80.0),
        WidgetKind::Label => (120.0, 30.0),
        WidgetKind::Button => (100.0, 40.0),
        WidgetKind::Indicator => (40.0, 40.0),
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_panel() {
        let panel = PanelModel::new("My Panel");
        assert_eq!(panel.name, "My Panel");
        assert!(panel.widgets.is_empty());
    }

    #[test]
    fn test_add_widget() {
        let mut panel = PanelModel::new("test");
        let id = panel.add_widget(WidgetKind::Toggle, "Switch");
        assert_eq!(id, 1);
        assert_eq!(panel.widgets.len(), 1);

        let w = panel.get_widget(id).unwrap();
        assert_eq!(w.label, "Switch");
        assert_eq!(w.kind, WidgetKind::Toggle);
    }

    #[test]
    fn test_remove_widget() {
        let mut panel = PanelModel::new("test");
        let id1 = panel.add_widget(WidgetKind::Button, "Btn1");
        let id2 = panel.add_widget(WidgetKind::Label, "Lbl");

        assert!(panel.remove_widget(id1));
        assert_eq!(panel.widgets.len(), 1);
        assert!(panel.get_widget(id1).is_none());
        assert!(panel.get_widget(id2).is_some());

        // Removing again returns false.
        assert!(!panel.remove_widget(id1));
    }

    #[test]
    fn test_widget_kind_serde_roundtrip() {
        let kinds = vec![
            WidgetKind::Toggle,
            WidgetKind::Slider {
                min: 0.0,
                max: 100.0,
                step: 0.5,
            },
            WidgetKind::Gauge {
                min: -10.0,
                max: 10.0,
            },
            WidgetKind::Label,
            WidgetKind::Button,
            WidgetKind::Indicator,
        ];

        for kind in &kinds {
            let json = serde_json::to_string(kind).unwrap();
            let back: WidgetKind = serde_json::from_str(&json).unwrap();
            assert_eq!(&back, kind, "roundtrip failed for {kind:?}");
        }
    }

    #[test]
    fn test_panel_model_serde_roundtrip() {
        let mut panel = PanelModel::new("Roundtrip");
        panel.add_widget(WidgetKind::Toggle, "SW1");
        panel.add_widget(
            WidgetKind::Slider {
                min: 0.0,
                max: 1.0,
                step: 0.01,
            },
            "Volume",
        );

        // Add a binding to the first widget.
        panel
            .get_widget_mut(1)
            .unwrap()
            .bindings
            .push(ChannelBinding {
                direction: BindingDirection::Output,
                topic: "led/enable".to_string(),
            });

        let json = serde_json::to_string_pretty(&panel).unwrap();
        let back: PanelModel = serde_json::from_str(&json).unwrap();

        assert_eq!(back.name, "Roundtrip");
        assert_eq!(back.widgets.len(), 2);
        // Widget id=1 (Toggle) has one schema-derived binding plus the
        // pushed one = 2. Both directions are Output (Toggle's schema is
        // Output; the pushed binding is also Output).
        assert_eq!(back.widgets[0].bindings.len(), 2);
        assert!(back.widgets[0]
            .bindings
            .iter()
            .all(|b| b.direction == BindingDirection::Output));
    }

    #[test]
    fn test_unique_widget_ids() {
        let mut panel = PanelModel::new("ids");
        let mut ids = Vec::new();
        for i in 0..20 {
            ids.push(panel.add_widget(WidgetKind::Button, &format!("btn{i}")));
        }

        // Remove some, add more — ids should never collide.
        panel.remove_widget(ids[3]);
        panel.remove_widget(ids[7]);

        let new1 = panel.add_widget(WidgetKind::Label, "new1");
        let new2 = panel.add_widget(WidgetKind::Label, "new2");

        // All remaining ids must be unique.
        let mut all_ids: Vec<u32> = panel.widgets.iter().map(|w| w.id).collect();
        all_ids.sort_unstable();
        let before = all_ids.len();
        all_ids.dedup();
        assert_eq!(all_ids.len(), before, "duplicate widget ids found");

        // New ids should be greater than the highest previously issued id.
        assert!(new1 > *ids.last().unwrap());
        assert!(new2 > new1);
    }
}
