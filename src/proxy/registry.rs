use super::adapter::CommandAdapter;
use super::command_ast::CommandAst;
use crate::adapters;

/// Adapter registry. Adapters are evaluated in insertion order, first match wins.
/// The generic adapter is always last so it acts as fallback.
pub struct AdapterRegistry {
    adapters: Vec<Box<dyn CommandAdapter>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self {
            adapters: Vec::new(),
        }
    }

    pub fn register(&mut self, adapter: Box<dyn CommandAdapter>) {
        self.adapters.push(adapter);
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.adapters.iter().map(|a| a.name()).collect()
    }

    /// Look up an adapter by name (case-insensitive).
    pub fn by_name(&self, name: &str) -> Option<&dyn CommandAdapter> {
        let lower = name.to_ascii_lowercase();
        self.adapters
            .iter()
            .map(|a| a.as_ref())
            .find(|a| a.name().eq_ignore_ascii_case(&lower))
    }

    /// Pick the first adapter that matches the AST. Falls back to the generic
    /// adapter (registered last).
    pub fn pick(&self, ast: &CommandAst) -> &dyn CommandAdapter {
        for adapter in &self.adapters {
            if adapter.name() == "generic" {
                continue;
            }
            if adapter.matches(ast) {
                return adapter.as_ref();
            }
        }
        self.adapters
            .iter()
            .map(|a| a.as_ref())
            .find(|a| a.name() == "generic")
            .expect("generic adapter must be registered")
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        default_registry()
    }
}

impl AdapterRegistry {
    pub fn adapters(&self) -> impl Iterator<Item = &dyn CommandAdapter> {
        self.adapters.iter().map(|a| a.as_ref())
    }
}

/// Return the default adapter registry. The order matters: more specific
/// adapters are registered first, the generic fallback is registered last.
pub fn default_registry() -> AdapterRegistry {
    let mut registry = AdapterRegistry::new();
    // Specific adapters land in the proxy-adapters PR; the generic adapter is
    // always present and acts as the fallback.
    adapters::register_specific(&mut registry);
    registry.register(Box::new(adapters::generic::GenericAdapter));
    registry
}

#[cfg(test)]
mod tests {
    use super::super::command_ast::build_ast;
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn generic_fallback_is_selected_when_no_specific_match() {
        let registry = default_registry();
        let ast = build_ast(
            &["echo".to_string(), "hello".to_string()],
            PathBuf::from("/tmp"),
            false,
            None,
        );
        let adapter = registry.pick(&ast);
        assert_eq!(adapter.name(), "generic");
    }

    #[test]
    fn registry_lists_at_least_generic() {
        let registry = default_registry();
        assert!(registry.names().contains(&"generic"));
    }

    #[test]
    fn by_name_is_case_insensitive() {
        let registry = default_registry();
        assert!(registry.by_name("Generic").is_some());
        assert!(registry.by_name("not-real").is_none());
    }
}
