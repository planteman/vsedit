//! Progress indicators.

use std::fmt;

/// Errors returned by fallible progress operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressError {
    TaskNotFound(u64),
    AlreadyComplete(u64),
    NotCancellable(u64),
}

impl fmt::Display for ProgressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TaskNotFound(id) => write!(f, "task {id} not found"),
            Self::AlreadyComplete(id) => write!(f, "task {id} is already complete"),
            Self::NotCancellable(id) => write!(f, "task {id} is not cancellable"),
        }
    }
}

/// Where a progress indicator is displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressLocation {
    Notification,
    Window,
    SourceControl,
    StatusBar,
}

impl fmt::Display for ProgressLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Notification => write!(f, "Notification"),
            Self::Window => write!(f, "Window"),
            Self::SourceControl => write!(f, "Source Control"),
            Self::StatusBar => write!(f, "Status Bar"),
        }
    }
}

/// Options for starting a progress task.
#[derive(Debug, Clone)]
pub struct ProgressOptions {
    pub location: ProgressLocation,
    pub title: Option<String>,
    pub cancellable: bool,
}

/// Builder for constructing [`ProgressOptions`].
#[derive(Debug)]
pub struct ProgressOptionsBuilder {
    location: ProgressLocation,
    title: Option<String>,
    cancellable: bool,
}

impl ProgressOptionsBuilder {
    pub fn new(location: ProgressLocation) -> Self {
        Self {
            location,
            title: None,
            cancellable: false,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn cancellable(mut self, cancellable: bool) -> Self {
        self.cancellable = cancellable;
        self
    }

    pub fn build(self) -> ProgressOptions {
        ProgressOptions {
            location: self.location,
            title: self.title,
            cancellable: self.cancellable,
        }
    }
}

/// Current progress report.
#[derive(Debug, Clone)]
pub struct ProgressReport {
    pub message: Option<String>,
    pub increment: Option<f64>,
    pub total: f64,
}

/// A running progress task.
#[derive(Debug, Clone)]
pub struct ProgressTask {
    pub id: u64,
    pub options: ProgressOptions,
    pub report: ProgressReport,
    pub cancelled: bool,
    pub done: bool,
}

impl ProgressTask {
    /// Returns `true` if no increments have been reported yet.
    pub fn is_indeterminate(&self) -> bool {
        self.report.increment.is_none()
    }
}

impl fmt::Display for ProgressTask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let title = self.options.title.as_deref().unwrap_or("Untitled");
        let pct = self.report.total.min(100.0).max(0.0);
        write!(f, "[{title}] {pct:.0}%")
    }
}

/// Service for managing progress tasks.
pub struct ProgressService {
    tasks: Vec<ProgressTask>,
    next_id: u64,
}

impl ProgressService {
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            next_id: 1,
        }
    }

    pub fn start(&mut self, options: ProgressOptions) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.tasks.push(ProgressTask {
            id,
            options,
            report: ProgressReport {
                message: None,
                increment: None,
                total: 0.0,
            },
            cancelled: false,
            done: false,
        });
        id
    }

    pub fn report(&mut self, id: u64, message: Option<String>, increment: Option<f64>) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.report.message = message;
            if let Some(inc) = increment {
                task.report.total += inc;
                task.report.increment = Some(inc);
            }
        }
    }

    pub fn cancel(&mut self, id: u64) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.cancelled = true;
            task.done = true;
        }
    }

    pub fn complete(&mut self, id: u64) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.done = true;
        }
    }

    pub fn get_task(&self, id: u64) -> Option<&ProgressTask> {
        self.tasks.iter().find(|t| t.id == id)
    }

    pub fn active_count(&self) -> usize {
        self.tasks.iter().filter(|t| !t.done).count()
    }

    pub fn is_cancelled(&self, id: u64) -> bool {
        self.tasks
            .iter()
            .find(|t| t.id == id)
            .map_or(false, |t| t.cancelled)
    }

    /// Cancel a task, returning an error if the task is not found,
    /// already complete, or not cancellable.
    pub fn try_cancel(&mut self, id: u64) -> Result<(), ProgressError> {
        let task = self
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or(ProgressError::TaskNotFound(id))?;
        if task.done {
            return Err(ProgressError::AlreadyComplete(id));
        }
        if !task.options.cancellable {
            return Err(ProgressError::NotCancellable(id));
        }
        task.cancelled = true;
        task.done = true;
        Ok(())
    }

    /// Returns all tasks that are not yet done.
    pub fn get_active_tasks(&self) -> Vec<&ProgressTask> {
        self.tasks.iter().filter(|t| !t.done).collect()
    }

    /// Removes all completed (done) tasks and returns how many were removed.
    pub fn remove_completed(&mut self) -> usize {
        let before = self.tasks.len();
        self.tasks.retain(|t| !t.done);
        before - self.tasks.len()
    }

    /// Returns the percentage (0.0–100.0) for a task, clamped.
    /// Returns `None` if the task is not found.
    pub fn percentage(&self, id: u64) -> Option<f64> {
        self.get_task(id)
            .map(|t| t.report.total.min(100.0).max(0.0))
    }

    /// Returns the total number of tasks (active and done).
    pub fn total_task_count(&self) -> usize {
        self.tasks.len()
    }
}

impl Default for ProgressService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(loc: ProgressLocation) -> ProgressOptions {
        ProgressOptions {
            location: loc,
            title: Some("Test".to_string()),
            cancellable: true,
        }
    }

    #[test]
    fn start_and_complete() {
        let mut svc = ProgressService::new();
        let id = svc.start(opts(ProgressLocation::Notification));
        assert_eq!(svc.active_count(), 1);
        svc.complete(id);
        assert_eq!(svc.active_count(), 0);
        assert!(svc.get_task(id).unwrap().done);
    }

    #[test]
    fn report_progress() {
        let mut svc = ProgressService::new();
        let id = svc.start(opts(ProgressLocation::StatusBar));
        svc.report(id, Some("step 1".to_string()), Some(25.0));
        svc.report(id, Some("step 2".to_string()), Some(25.0));
        let task = svc.get_task(id).unwrap();
        assert_eq!(task.report.total, 50.0);
        assert_eq!(task.report.message.as_deref(), Some("step 2"));
    }

    #[test]
    fn cancel_task() {
        let mut svc = ProgressService::new();
        let id = svc.start(opts(ProgressLocation::Window));
        assert!(!svc.is_cancelled(id));
        svc.cancel(id);
        assert!(svc.is_cancelled(id));
        assert_eq!(svc.active_count(), 0);
    }

    #[test]
    fn try_cancel_success() {
        let mut svc = ProgressService::new();
        let id = svc.start(opts(ProgressLocation::Notification));
        assert!(svc.try_cancel(id).is_ok());
        assert!(svc.is_cancelled(id));
    }

    #[test]
    fn try_cancel_not_cancellable() {
        let mut svc = ProgressService::new();
        let id = svc.start(ProgressOptions {
            location: ProgressLocation::Window,
            title: Some("NC".to_string()),
            cancellable: false,
        });
        assert_eq!(svc.try_cancel(id), Err(ProgressError::NotCancellable(id)));
    }

    #[test]
    fn try_cancel_already_complete() {
        let mut svc = ProgressService::new();
        let id = svc.start(opts(ProgressLocation::StatusBar));
        svc.complete(id);
        assert_eq!(svc.try_cancel(id), Err(ProgressError::AlreadyComplete(id)));
    }

    #[test]
    fn try_cancel_task_not_found() {
        let mut svc = ProgressService::new();
        assert_eq!(svc.try_cancel(999), Err(ProgressError::TaskNotFound(999)));
    }

    #[test]
    fn get_active_tasks() {
        let mut svc = ProgressService::new();
        let id1 = svc.start(opts(ProgressLocation::Notification));
        let _id2 = svc.start(opts(ProgressLocation::Window));
        svc.complete(id1);
        let active = svc.get_active_tasks();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, _id2);
    }

    #[test]
    fn remove_completed_tasks() {
        let mut svc = ProgressService::new();
        let id1 = svc.start(opts(ProgressLocation::Notification));
        let _id2 = svc.start(opts(ProgressLocation::Window));
        svc.complete(id1);
        let removed = svc.remove_completed();
        assert_eq!(removed, 1);
        assert_eq!(svc.total_task_count(), 1);
    }

    #[test]
    fn percentage_calculation() {
        let mut svc = ProgressService::new();
        let id = svc.start(opts(ProgressLocation::StatusBar));
        assert_eq!(svc.percentage(id), Some(0.0));
        svc.report(id, None, Some(40.0));
        assert_eq!(svc.percentage(id), Some(40.0));
        svc.report(id, None, Some(80.0));
        // Clamped to 100
        assert_eq!(svc.percentage(id), Some(100.0));
    }

    #[test]
    fn percentage_not_found() {
        let svc = ProgressService::new();
        assert_eq!(svc.percentage(42), None);
    }

    #[test]
    fn is_indeterminate() {
        let mut svc = ProgressService::new();
        let id = svc.start(opts(ProgressLocation::Notification));
        assert!(svc.get_task(id).unwrap().is_indeterminate());
        svc.report(id, Some("working".to_string()), Some(10.0));
        assert!(!svc.get_task(id).unwrap().is_indeterminate());
    }

    #[test]
    fn progress_options_builder() {
        let opts = ProgressOptionsBuilder::new(ProgressLocation::SourceControl)
            .title("Building")
            .cancellable(true)
            .build();
        assert_eq!(opts.location, ProgressLocation::SourceControl);
        assert_eq!(opts.title.as_deref(), Some("Building"));
        assert!(opts.cancellable);
    }

    #[test]
    fn progress_options_builder_defaults() {
        let opts = ProgressOptionsBuilder::new(ProgressLocation::Notification).build();
        assert_eq!(opts.location, ProgressLocation::Notification);
        assert!(opts.title.is_none());
        assert!(!opts.cancellable);
    }

    #[test]
    fn display_progress_location() {
        assert_eq!(ProgressLocation::Notification.to_string(), "Notification");
        assert_eq!(ProgressLocation::SourceControl.to_string(), "Source Control");
        assert_eq!(ProgressLocation::StatusBar.to_string(), "Status Bar");
        assert_eq!(ProgressLocation::Window.to_string(), "Window");
    }

    #[test]
    fn display_progress_task() {
        let mut svc = ProgressService::new();
        let id = svc.start(opts(ProgressLocation::Window));
        assert_eq!(svc.get_task(id).unwrap().to_string(), "[Test] 0%");
        svc.report(id, None, Some(75.0));
        assert_eq!(svc.get_task(id).unwrap().to_string(), "[Test] 75%");
    }

    #[test]
    fn display_progress_error() {
        assert_eq!(ProgressError::TaskNotFound(1).to_string(), "task 1 not found");
        assert_eq!(
            ProgressError::AlreadyComplete(2).to_string(),
            "task 2 is already complete"
        );
        assert_eq!(
            ProgressError::NotCancellable(3).to_string(),
            "task 3 is not cancellable"
        );
    }

    #[test]
    fn total_task_count() {
        let mut svc = ProgressService::new();
        assert_eq!(svc.total_task_count(), 0);
        let id1 = svc.start(opts(ProgressLocation::Notification));
        let _id2 = svc.start(opts(ProgressLocation::Window));
        assert_eq!(svc.total_task_count(), 2);
        svc.complete(id1);
        assert_eq!(svc.total_task_count(), 2);
    }
}
