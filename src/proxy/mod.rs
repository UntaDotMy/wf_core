#![allow(dead_code)]

pub mod adapter;
pub mod command_ast;
pub mod event_log;
pub mod raw_store;
pub mod registry;
pub mod render;
pub mod run;
pub mod safety;
pub mod shell;
pub mod shim;
pub mod token_meter;

pub use adapter::OutputBudget;
pub use command_ast::build_ast;
pub use event_log::{gain_events_path, read_events, GainEventV2};
pub use raw_store::{
    find_raw_dir, now_unix_ms, parse_duration_ms, prune_older_than, raw_store_root, ProxyTarget,
};
pub use registry::default_registry;
pub use run::{list_raw_runs, print_raw, print_raw_path, run_proxy, RunOptions};
pub use safety::{is_destructive, is_interactive_command};
pub use shim::{
    dispatch_command, install_shims, install_shims_with_binary, list_shims, print_shell_init,
    shim_dir, shim_doctor, uninstall_shims, DispatchOptions, ShellInitOptions, ShimInstallOptions,
};
