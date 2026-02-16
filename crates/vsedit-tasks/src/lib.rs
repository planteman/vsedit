//! VS Code tasks.json parsing, task execution, problem matching, and auto-detection.

pub mod definition;
pub mod detect;
pub mod execution;
pub mod problem_matcher;
pub mod runner;
pub mod variables;

pub use definition::*;
pub use detect::detect_tasks;
pub use execution::*;
pub use problem_matcher::*;
pub use runner::*;
pub use variables::*;
