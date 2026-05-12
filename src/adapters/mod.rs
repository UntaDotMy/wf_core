pub mod generic;

use crate::proxy::registry::AdapterRegistry;

/// Hook called by the registry to register every specific adapter. In the
/// `feat/proxy-core` PR this is a no-op so only the generic fallback is wired
/// up; the `feat/proxy-adapters` PR fills in specific adapters here.
pub fn register_specific(_registry: &mut AdapterRegistry) {}
