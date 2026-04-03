//! Control panel configuration model.
//!
//! A [`PanelModel`] describes a collection of UI widgets that bind directly
//! to pubsub topics.  The model is saved/loaded as JSON and is independent
//! of the dataflow graph.

use serde::{Deserialize, Serialize};

use crate::dataflow::block::PortKind;

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

/// Position of a widget on the panel canvas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// Size of a widget on the panel canvas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

// ---------------------------------------------------------------------------
// Channel binding
// ---------------------------------------------------------------------------

/// Direction of data flow between a widget and a pubsub topic.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChannelDirection {
    Input,
    Output,
}

/// Binds a widget port to a pubsub topic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelBinding {
    pub topic: String,
    pub direction: ChannelDirection,
    pub port_kind: PortKind,
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// The type of UI control a widget represents.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WidgetKind {
    /// Boolean on/off — publishes Float(0.0 / 1.0).
    Toggle,
    /// Range input — publishes Float.
    Slider { min: f64, max: f64, step: f64 },
    /// Read-only value display with range.
    Gauge { min: f64, max: f64 },
    /// Text display — subscribes to Text or Float.
    Label,
    /// Momentary press — publishes Float(1.0) on press.
    Button,
    /// Boolean light — subscribes to Float (>0.5 = on).
    Indicator,
}

/// A single UI control element on a panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Widget {
    pub id: u32,
    pub kind: WidgetKind,
    pub label: String,
    pub position: Position,
    pub size: Size,
    pub channels: Vec<ChannelBinding>,
}

// ---------------------------------------------------------------------------
// PanelModel
// ---------------------------------------------------------------------------

/// Top-level panel configuration — a named collection of widgets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelModel {
    pub name: String,
    pub widgets: Vec<Widget>,
}

impl PanelModel {
    /// Create an empty panel with the given name.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            widgets: Vec::new(),
        }
    }

    /// Add a widget to the panel, assigning it an auto-incremented id.
    ///
    /// Returns the assigned id.
    pub fn add_widget(&mut self, mut widget: Widget) -> u32 {
        let next_id = self.widgets.iter().map(|w| w.id).max().unwrap_or(0) + 1;
        widget.id = next_id;
        self.widgets.push(widget);
        next_id
    }

    /// Remove a widget by id.  Returns `true` if the widget was found and
    /// removed, `false` otherwise.
    pub fn remove_widget(&mut self, widget_id: u32) -> bool {
        let before = self.widgets.len();
        self.widgets.retain(|w| w.id != widget_id);
        self.widgets.len() < before
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a minimal widget for testing.
    fn make_widget(kind: WidgetKind, label: &str) -> Widget {
        Widget {
            id: 0, // will be overwritten by add_widget
            kind,
            label: label.to_string(),
            position: Position { x: 10.0, y: 20.0 },
            size: Size {
                width: 100.0,
                height: 40.0,
            },
            channels: vec![],
        }
    }

    #[test]
    fn round_trip_mixed_widgets() {
        let mut panel = PanelModel::new("test-panel");
        panel.add_widget(make_widget(WidgetKind::Toggle, "Enable"));
        panel.add_widget(make_widget(
            WidgetKind::Slider {
                min: 0.0,
                max: 100.0,
                step: 1.0,
            },
            "Speed",
        ));
        panel.add_widget(make_widget(
            WidgetKind::Gauge {
                min: 0.0,
                max: 200.0,
            },
            "RPM",
        ));
        panel.add_widget(make_widget(WidgetKind::Label, "Status"));
        panel.add_widget(make_widget(WidgetKind::Button, "E-Stop"));
        panel.add_widget(make_widget(WidgetKind::Indicator, "Alarm"));

        let json = serde_json::to_string_pretty(&panel).unwrap();
        let deserialized: PanelModel = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, "test-panel");
        assert_eq!(deserialized.widgets.len(), 6);
        assert_eq!(deserialized.widgets[0].label, "Enable");
        assert_eq!(deserialized.widgets[5].label, "Alarm");
    }

    #[test]
    fn add_widget_auto_increments_ids() {
        let mut panel = PanelModel::new("ids");

        let id1 = panel.add_widget(make_widget(WidgetKind::Toggle, "A"));
        let id2 = panel.add_widget(make_widget(WidgetKind::Button, "B"));
        let id3 = panel.add_widget(make_widget(WidgetKind::Label, "C"));

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);

        assert_eq!(panel.widgets[0].id, 1);
        assert_eq!(panel.widgets[1].id, 2);
        assert_eq!(panel.widgets[2].id, 3);
    }

    #[test]
    fn add_widget_after_removal_uses_max_plus_one() {
        let mut panel = PanelModel::new("gap");

        panel.add_widget(make_widget(WidgetKind::Toggle, "A")); // id 1
        panel.add_widget(make_widget(WidgetKind::Toggle, "B")); // id 2
        panel.add_widget(make_widget(WidgetKind::Toggle, "C")); // id 3

        panel.remove_widget(2);
        let id4 = panel.add_widget(make_widget(WidgetKind::Toggle, "D"));

        // next id should be max(1,3)+1 = 4, not 3
        assert_eq!(id4, 4);
    }

    #[test]
    fn remove_widget_returns_correct_bool() {
        let mut panel = PanelModel::new("rm");
        panel.add_widget(make_widget(WidgetKind::Toggle, "A"));

        assert!(panel.remove_widget(1));
        assert!(!panel.remove_widget(1)); // already removed
        assert!(!panel.remove_widget(999)); // never existed
    }

    #[test]
    fn deserialize_from_hand_written_json() {
        let json = r#"{
            "name": "my-panel",
            "widgets": [
                {
                    "id": 42,
                    "kind": { "type": "Slider", "min": 0.0, "max": 10.0, "step": 0.5 },
                    "label": "Volume",
                    "position": { "x": 5.0, "y": 15.0 },
                    "size": { "width": 200.0, "height": 30.0 },
                    "channels": [
                        {
                            "topic": "audio/volume",
                            "direction": "Output",
                            "port_kind": "Float"
                        }
                    ]
                },
                {
                    "id": 43,
                    "kind": { "type": "Indicator" },
                    "label": "Clip",
                    "position": { "x": 5.0, "y": 50.0 },
                    "size": { "width": 30.0, "height": 30.0 },
                    "channels": [
                        {
                            "topic": "audio/clip",
                            "direction": "Input",
                            "port_kind": "Float"
                        }
                    ]
                }
            ]
        }"#;

        let panel: PanelModel = serde_json::from_str(json).unwrap();
        assert_eq!(panel.name, "my-panel");
        assert_eq!(panel.widgets.len(), 2);

        let slider = &panel.widgets[0];
        assert_eq!(slider.id, 42);
        assert_eq!(slider.label, "Volume");
        assert_eq!(slider.channels.len(), 1);
        assert_eq!(slider.channels[0].topic, "audio/volume");
        assert_eq!(slider.channels[0].direction, ChannelDirection::Output);

        let indicator = &panel.widgets[1];
        assert_eq!(indicator.id, 43);
        assert_eq!(indicator.channels[0].direction, ChannelDirection::Input);
    }

    #[test]
    fn widget_kind_serde_tag_format() {
        // Toggle — no fields
        let json = serde_json::to_value(WidgetKind::Toggle).unwrap();
        assert_eq!(json["type"], "Toggle");

        // Slider — with fields
        let json = serde_json::to_value(WidgetKind::Slider {
            min: 1.0,
            max: 10.0,
            step: 0.5,
        })
        .unwrap();
        assert_eq!(json["type"], "Slider");
        assert_eq!(json["min"], 1.0);
        assert_eq!(json["max"], 10.0);
        assert_eq!(json["step"], 0.5);

        // Gauge — with fields
        let json = serde_json::to_value(WidgetKind::Gauge {
            min: 0.0,
            max: 200.0,
        })
        .unwrap();
        assert_eq!(json["type"], "Gauge");
        assert_eq!(json["min"], 0.0);
        assert_eq!(json["max"], 200.0);

        // Label — no fields
        let json = serde_json::to_value(WidgetKind::Label).unwrap();
        assert_eq!(json["type"], "Label");

        // Button — no fields
        let json = serde_json::to_value(WidgetKind::Button).unwrap();
        assert_eq!(json["type"], "Button");

        // Indicator — no fields
        let json = serde_json::to_value(WidgetKind::Indicator).unwrap();
        assert_eq!(json["type"], "Indicator");
    }

    #[test]
    fn channel_binding_round_trip() {
        let binding = ChannelBinding {
            topic: "motor/speed".to_string(),
            direction: ChannelDirection::Output,
            port_kind: PortKind::Float,
        };
        let json = serde_json::to_string(&binding).unwrap();
        let rt: ChannelBinding = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.topic, "motor/speed");
        assert_eq!(rt.direction, ChannelDirection::Output);
    }

    #[test]
    fn empty_panel_new() {
        let panel = PanelModel::new("blank");
        assert_eq!(panel.name, "blank");
        assert!(panel.widgets.is_empty());
    }
}
