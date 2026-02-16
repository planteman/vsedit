//! Progress indicators.

/// Where a progress indicator is displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressLocation {
    Notification,
    Window,
    SourceControl,
    StatusBar,
}

/// Options for starting a progress task.
#[derive(Debug, Clone)]
pub struct ProgressOptions {
    pub location: ProgressLocation,
    pub title: Option<String>,
    pub cancellable: bool,
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
}
