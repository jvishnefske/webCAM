#[test]
fn parse_serialize_full_example() {
    let input = "\
@target(rp2040)
block sensor: adc_source(channel = 0)
block amp: gain(2.5)

@target(host)
block display: plot(\"Sensor Output\")

sensor.out -> amp.input
amp.out -> display.input
";
    let graph = parser::parse(input).unwrap();
    assert_eq!(graph.blocks.len(), 3);
    assert_eq!(graph.connections.len(), 2);
    assert_eq!(graph.blocks[0].id, "sensor");
    assert_eq!(graph.blocks[0].block_type, "adc_source");
    assert_eq!(graph.blocks[0].annotations.len(), 1);
    assert_eq!(graph.blocks[0].annotations[0].name, "target");
    assert_eq!(graph.blocks[2].id, "display");
    assert_eq!(graph.blocks[2].block_type, "plot");
    assert_eq!(graph.connections[0].from_block, "sensor");
    assert_eq!(graph.connections[0].to_block, "amp");

    // Round-trip: serialize normalizes (sorts connections), so re-serialize should be stable
    let output = parser::serialize(&graph);
    let graph2 = parser::parse(&output).unwrap();
    let output2 = parser::serialize(&graph2);
    assert_eq!(output, output2, "serialized form should be stable after one round-trip");
}

#[test]
fn parse_state_machine_structured_config() {
    let input = "\
block ctrl: state_machine {
  initial: idle
  states: [idle, running, error]
  transitions: [{ from: idle, to: running, guard: 0 }, { from: running, to: error, guard: 1 }]
}
";
    let graph = parser::parse(input).unwrap();
    assert_eq!(graph.blocks.len(), 1);
    match &graph.blocks[0].config {
        parser::ast::Config::Structured(entries) => {
            assert_eq!(entries[0].0, "initial");
            assert_eq!(entries[0].1, parser::ast::Value::Ident("idle".into()));
            assert_eq!(entries.len(), 3);
        }
        _ => panic!("expected structured config"),
    }
}

#[test]
fn parse_all_config_forms() {
    let input = "\
block a: constant(42.0)
block b: pwm_sink(channel = 0, frequency = 1000)
block c: add
block d: state_machine {
  initial: idle
}
";
    let graph = parser::parse(input).unwrap();
    assert!(matches!(graph.blocks[0].config, parser::ast::Config::Positional(_)));
    assert!(matches!(graph.blocks[1].config, parser::ast::Config::Named(_)));
    assert!(matches!(graph.blocks[2].config, parser::ast::Config::Empty));
    assert!(matches!(graph.blocks[3].config, parser::ast::Config::Structured(_)));
}

#[test]
fn parse_error_gives_location() {
    let err = parser::parse("block :\n").unwrap_err();
    assert_eq!(err.line, 1);
    assert!(err.column > 0);
    assert!(!err.expected.is_empty());
}
