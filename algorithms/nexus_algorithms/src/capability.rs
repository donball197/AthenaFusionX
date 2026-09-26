//! Discoverable cognitive capability catalog.
//!
//! This module describes capabilities without executing them.
//! It intentionally does not depend on runtime_core or on the individual
//! algorithm implementation crates. The registry is metadata; execution and
//! authorization remain owned by the runtime.

/// Distinguishes model-backed cognitive capabilities from deterministic
/// algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityKind {
    ModelBacked,
    Deterministic,
}

/// Static description of one cognitive capability.
///
/// This descriptor deliberately contains no model identity or routing tier.
/// Model selection remains an internal concern of the routing/provider layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityDescriptor {
    pub name: &'static str,
    pub kind: CapabilityKind,
    pub description: &'static str,
}

/// Read-only catalog of capabilities known to NEXUS.
///
/// The registry describes capabilities; it does not execute them, grant tool
/// authority, or bypass Cortex/critic/completion gates.
pub struct CapabilityRegistry;

impl CapabilityRegistry {
    const ALL: [CapabilityDescriptor; 12] = [
        CapabilityDescriptor {
            name: "plan",
            kind: CapabilityKind::ModelBacked,
            description: "Decompose a directive into a deterministic TaskGraph.",
        },
        CapabilityDescriptor {
            name: "reasoning",
            kind: CapabilityKind::ModelBacked,
            description: "Generate competing hypotheses and reasoning strategies.",
        },
        CapabilityDescriptor {
            name: "action_planning",
            kind: CapabilityKind::ModelBacked,
            description: "Translate an approved strategy into a native action proposal.",
        },
        CapabilityDescriptor {
            name: "critique",
            kind: CapabilityKind::ModelBacked,
            description: "Critique a proposed action against mission and evidence.",
        },
        CapabilityDescriptor {
            name: "verify",
            kind: CapabilityKind::ModelBacked,
            description: "Assess whether execution evidence independently proves the objective.",
        },
        CapabilityDescriptor {
            name: "synthesize",
            kind: CapabilityKind::ModelBacked,
            description: "Synthesize gathered source evidence into an architectural report.",
        },
        CapabilityDescriptor {
            name: "evaluate",
            kind: CapabilityKind::ModelBacked,
            description: "Evaluate physical evidence against the active strategy.",
        },
        CapabilityDescriptor {
            name: "reflect",
            kind: CapabilityKind::ModelBacked,
            description: "Reflect on a failed strategy and identify a possible pivot.",
        },
        CapabilityDescriptor {
            name: "action_translation",
            kind: CapabilityKind::ModelBacked,
            description: "Translate structured intent into a native AgentAction representation.",
        },
        CapabilityDescriptor {
            name: "reconnaissance",
            kind: CapabilityKind::Deterministic,
            description: "Derive unread source-read strategies from physical search evidence.",
        },
        CapabilityDescriptor {
            name: "strategy_compilation",
            kind: CapabilityKind::Deterministic,
            description: "Compile explicitly classified directive conditions into strategies.",
        },
        CapabilityDescriptor {
            name: "action_identity",
            kind: CapabilityKind::Deterministic,
            description: "Calculate canonical anti-loop identities for native actions.",
        },
    ];

    /// Return the complete read-only capability catalog.
    pub const fn all() -> &'static [CapabilityDescriptor] {
        &Self::ALL
    }

    /// Find a capability by its stable name.
    pub fn get(name: &str) -> Option<&'static CapabilityDescriptor> {
        Self::ALL.iter().find(|capability| capability.name == name)
    }

    /// Render the complete capability catalog as stable, read-only planner
    /// context.
    ///
    /// This deliberately exposes capability metadata only. It does not expose
    /// model identities, routing tiers, providers, credentials, tool
    /// authority, or mutation authority.
    pub fn render_catalog() -> String {
        let mut rendered = String::from("AVAILABLE COGNITIVE CAPABILITIES\n");

        for capability in Self::ALL {
            let kind = match capability.kind {
                CapabilityKind::ModelBacked => "model-backed",
                CapabilityKind::Deterministic => "deterministic",
            };

            rendered.push_str("\n");
            rendered.push_str(capability.name);
            rendered.push_str("\n");
            rendered.push_str("  kind: ");
            rendered.push_str(kind);
            rendered.push_str("\n");
            rendered.push_str("  description: ");
            rendered.push_str(capability.description);
            rendered.push('\n');
        }

        rendered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_all_current_capabilities() {
        assert_eq!(CapabilityRegistry::all().len(), 12);

        let names: Vec<&str> =
            CapabilityRegistry::all().iter().map(|capability| capability.name).collect();

        assert_eq!(
            names,
            vec![
                "plan",
                "reasoning",
                "action_planning",
                "critique",
                "verify",
                "synthesize",
                "evaluate",
                "reflect",
                "action_translation",
                "reconnaissance",
                "strategy_compilation",
                "action_identity",
            ]
        );
    }

    #[test]
    fn capability_names_are_unique() {
        let capabilities = CapabilityRegistry::all();

        for (index, capability) in capabilities.iter().enumerate() {
            assert!(
                capabilities[index + 1..].iter().all(|other| other.name != capability.name),
                "duplicate capability name: {}",
                capability.name
            );
        }
    }

    #[test]
    fn model_backed_capabilities_are_registered() {
        for name in [
            "plan",
            "reasoning",
            "action_planning",
            "critique",
            "verify",
            "synthesize",
            "evaluate",
            "reflect",
            "action_translation",
        ] {
            let capability = CapabilityRegistry::get(name).unwrap();
            assert_eq!(capability.kind, CapabilityKind::ModelBacked);
        }
    }

    #[test]
    fn deterministic_algorithms_are_registered() {
        for name in ["reconnaissance", "strategy_compilation", "action_identity"] {
            let capability = CapabilityRegistry::get(name).unwrap();
            assert_eq!(capability.kind, CapabilityKind::Deterministic);
        }
    }

    #[test]
    fn lookup_returns_expected_descriptors() {
        let reasoning = CapabilityRegistry::get("reasoning").unwrap();
        assert_eq!(reasoning.kind, CapabilityKind::ModelBacked);
        assert!(!reasoning.description.is_empty());

        let reconnaissance = CapabilityRegistry::get("reconnaissance").unwrap();
        assert_eq!(reconnaissance.kind, CapabilityKind::Deterministic);
        assert!(!reconnaissance.description.is_empty());
    }

    #[test]
    fn render_catalog_contains_all_capabilities() {
        let catalog = CapabilityRegistry::render_catalog();

        for capability in CapabilityRegistry::all() {
            assert!(
                catalog.contains(capability.name),
                "catalog missing capability: {}",
                capability.name
            );
            assert!(
                catalog.contains(capability.description),
                "catalog missing description: {}",
                capability.name
            );
        }

        assert!(catalog.contains("kind: model-backed"));
        assert!(catalog.contains("kind: deterministic"));
    }

    #[test]
    fn render_catalog_does_not_expose_routing_or_model_details() {
        let catalog = CapabilityRegistry::render_catalog();

        assert!(!catalog.contains("ModelTier"));
        assert!(!catalog.contains("Heavy"));
        assert!(!catalog.contains("Light"));
        assert!(!catalog.contains("Planner"));
        assert!(!catalog.contains("gemini"));
        assert!(!catalog.contains("glm"));
        assert!(!catalog.contains("gemma"));
        assert!(!catalog.contains("provider"));
    }

    #[test]
    fn render_catalog_has_stable_order() {
        let catalog = CapabilityRegistry::render_catalog();

        let plan = catalog.find("\nplan\n").unwrap();
        let reasoning = catalog.find("\nreasoning\n").unwrap();
        let reconnaissance = catalog.find("\nreconnaissance\n").unwrap();
        let action_identity = catalog.find("\naction_identity\n").unwrap();

        assert!(plan < reasoning);
        assert!(reasoning < reconnaissance);
        assert!(reconnaissance < action_identity);
    }

    #[test]
    fn unknown_capability_is_not_registered() {
        assert!(CapabilityRegistry::get("not_a_real_capability").is_none());
    }
}
