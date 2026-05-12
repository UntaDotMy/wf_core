pub mod build_lint;
pub mod common;
pub mod curl;
pub mod files;
pub mod generic;
pub mod git;
pub mod logs;
pub mod package;
pub mod search;
pub mod tests;

use crate::proxy::registry::AdapterRegistry;

/// Hook called by the registry to register every specific adapter. The order
/// here matters: adapters are evaluated in insertion order and the first
/// `matches()` claim wins. Generic is registered separately as the fallback.
pub fn register_specific(registry: &mut AdapterRegistry) {
    registry.register(Box::new(tests::TestsAdapter));
    registry.register(Box::new(git::GitAdapter));
    registry.register(Box::new(search::SearchAdapter));
    registry.register(Box::new(build_lint::BuildLintAdapter));
    registry.register(Box::new(files::FilesAdapter));
    registry.register(Box::new(logs::LogsAdapter));
    registry.register(Box::new(curl::CurlAdapter));
    registry.register(Box::new(package::PackageAdapter));
}
