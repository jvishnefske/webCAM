use std::fmt;

use crate::ast::*;

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(v) => write!(f, "{v:?}"),
            Value::Text(s) => {
                write!(f, "\"")?;
                for ch in s.chars() {
                    match ch {
                        '"' => write!(f, "\\\"")?,
                        '\\' => write!(f, "\\\\")?,
                        '\n' => write!(f, "\\n")?,
                        '\t' => write!(f, "\\t")?,
                        c => write!(f, "{c}")?,
                    }
                }
                write!(f, "\"")
            }
            Value::Ident(s) => write!(f, "{s}"),
            Value::List(vs) => {
                write!(f, "[")?;
                for (i, v) in vs.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            Value::Map(entries) => {
                write!(f, "{{ ")?;
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, " }}")
            }
        }
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Config::Empty => Ok(()),
            Config::Positional(vs) => {
                write!(f, "(")?;
                for (i, v) in vs.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, ")")
            }
            Config::Named(ps) => {
                let mut sorted: Vec<_> = ps.clone();
                sorted.sort_by(|a, b| a.0.cmp(&b.0));
                write!(f, "(")?;
                for (i, (k, v)) in sorted.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k} = {v}")?;
                }
                write!(f, ")")
            }
            Config::Structured(entries) => {
                write!(f, " {{")?;
                for (k, v) in entries {
                    write!(f, "\n  {k}: {v}")?;
                }
                write!(f, "\n}}")
            }
        }
    }
}

impl fmt::Display for Annotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}", self.name)?;
        write!(f, "(")?;
        for (i, arg) in self.args.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{arg}")?;
        }
        write!(f, ")")
    }
}

impl fmt::Display for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{} -> {}.{}",
            self.from_block, self.from_port, self.to_block, self.to_port
        )
    }
}

impl fmt::Display for BlockDecl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for ann in &self.annotations {
            writeln!(f, "{ann}")?;
        }
        write!(f, "block {}: {}{}", self.id, self.block_type, self.config)
    }
}

impl fmt::Display for Graph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Helper: get the @target annotation value for a block
        fn target_annotation(b: &BlockDecl) -> Option<&str> {
            for ann in &b.annotations {
                if ann.name == "target" {
                    if let Some(Value::Ident(s)) = ann.args.first() {
                        return Some(s.as_str());
                    }
                }
            }
            None
        }

        for (i, block) in self.blocks.iter().enumerate() {
            if i > 0 {
                // Insert blank line if target annotation differs from previous block
                let prev_target = target_annotation(&self.blocks[i - 1]);
                let cur_target = target_annotation(block);
                if prev_target != cur_target {
                    writeln!(f)?;
                }
            }
            writeln!(f, "{block}")?;
        }

        if !self.connections.is_empty() {
            if !self.blocks.is_empty() {
                writeln!(f)?;
            }
            let mut sorted_conns: Vec<_> = self.connections.iter().collect();
            sorted_conns.sort_by(|a, b| {
                a.from_block.cmp(&b.from_block).then(a.from_port.cmp(&b.from_port))
            });
            for conn in sorted_conns {
                writeln!(f, "{conn}")?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::*;

    #[test]
    fn print_value_int() {
        assert_eq!(Value::Int(42).to_string(), "42");
    }

    #[test]
    fn print_value_float() {
        assert_eq!(Value::Float(42.0).to_string(), "42.0");
        assert_eq!(Value::Float(2.5).to_string(), "2.5");
    }

    #[test]
    fn print_value_text() {
        assert_eq!(Value::Text("hello".into()).to_string(), "\"hello\"");
    }

    #[test]
    fn print_value_text_escapes() {
        assert_eq!(Value::Text("a\"b".into()).to_string(), "\"a\\\"b\"");
        assert_eq!(Value::Text("a\\b".into()).to_string(), "\"a\\\\b\"");
    }

    #[test]
    fn print_value_ident() {
        assert_eq!(Value::Ident("idle".into()).to_string(), "idle");
    }

    #[test]
    fn print_value_list() {
        assert_eq!(
            Value::List(vec![Value::Int(1), Value::Int(2)]).to_string(),
            "[1, 2]"
        );
    }

    #[test]
    fn print_value_map() {
        let v = Value::Map(vec![
            ("from".into(), Value::Ident("idle".into())),
            ("to".into(), Value::Ident("running".into())),
        ]);
        assert_eq!(v.to_string(), "{ from: idle, to: running }");
    }

    #[test]
    fn print_connection() {
        let c = Connection {
            from_block: "a".into(),
            from_port: "out".into(),
            to_block: "b".into(),
            to_port: "input".into(),
        };
        assert_eq!(c.to_string(), "a.out -> b.input");
    }

    #[test]
    fn print_block_simple() {
        let b = BlockDecl {
            id: "c".into(),
            block_type: "constant".into(),
            config: Config::Positional(vec![Value::Float(42.0)]),
            annotations: vec![],
        };
        assert_eq!(b.to_string(), "block c: constant(42.0)");
    }

    #[test]
    fn print_block_no_config() {
        let b = BlockDecl {
            id: "sum".into(),
            block_type: "add".into(),
            config: Config::Empty,
            annotations: vec![],
        };
        assert_eq!(b.to_string(), "block sum: add");
    }

    #[test]
    fn print_block_named() {
        let b = BlockDecl {
            id: "m".into(),
            block_type: "pwm_sink".into(),
            config: Config::Named(vec![
                ("channel".into(), Value::Int(0)),
                ("frequency".into(), Value::Int(1000)),
            ]),
            annotations: vec![],
        };
        assert_eq!(
            b.to_string(),
            "block m: pwm_sink(channel = 0, frequency = 1000)"
        );
    }

    #[test]
    fn print_block_with_annotation() {
        let b = BlockDecl {
            id: "s".into(),
            block_type: "adc_source".into(),
            config: Config::Named(vec![("channel".into(), Value::Int(0))]),
            annotations: vec![Annotation {
                name: "target".into(),
                args: vec![Value::Ident("rp2040".into())],
            }],
        };
        assert_eq!(
            b.to_string(),
            "@target(rp2040)\nblock s: adc_source(channel = 0)"
        );
    }

    #[test]
    fn print_block_structured() {
        let b = BlockDecl {
            id: "ctrl".into(),
            block_type: "state_machine".into(),
            config: Config::Structured(vec![
                ("initial".into(), Value::Ident("idle".into())),
                (
                    "states".into(),
                    Value::List(vec![
                        Value::Ident("idle".into()),
                        Value::Ident("running".into()),
                    ]),
                ),
            ]),
            annotations: vec![],
        };
        assert_eq!(
            b.to_string(),
            "block ctrl: state_machine {\n  initial: idle\n  states: [idle, running]\n}"
        );
    }

    #[test]
    fn print_graph() {
        let g = Graph {
            blocks: vec![
                BlockDecl {
                    id: "a".into(),
                    block_type: "constant".into(),
                    config: Config::Positional(vec![Value::Float(1.0)]),
                    annotations: vec![],
                },
                BlockDecl {
                    id: "b".into(),
                    block_type: "gain".into(),
                    config: Config::Positional(vec![Value::Float(2.0)]),
                    annotations: vec![],
                },
            ],
            connections: vec![Connection {
                from_block: "a".into(),
                from_port: "out".into(),
                to_block: "b".into(),
                to_port: "input".into(),
            }],
        };
        assert_eq!(
            g.to_string(),
            "block a: constant(1.0)\nblock b: gain(2.0)\n\na.out -> b.input\n"
        );
    }
}
