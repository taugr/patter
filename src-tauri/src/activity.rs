//! Owned guards survive async work and prevent installation/quit racing with a new job.
use crate::store::Result;
use std::sync::{Arc, Mutex};
#[derive(Default, Clone)]
pub struct Activity(Arc<Mutex<Status>>);
#[derive(Default)]
struct Status {
    jobs: usize,
    exclusive: bool,
}
pub struct Guard {
    activity: Activity,
    exclusive: bool,
}
impl Activity {
    pub fn job(&self) -> Result<Guard> {
        self.enter(false)
    }
    pub fn exclusive(&self) -> Result<Guard> {
        self.enter(true)
    }
    fn enter(&self, exclusive: bool) -> Result<Guard> {
        let mut state = self.0.lock().map_err(|_| "Activity lock failed")?;
        if state.exclusive || (exclusive && state.jobs > 0) {
            return Err(
                "Finish recording, processing, or saving before updating or quitting.".into(),
            );
        }
        if exclusive {
            state.exclusive = true;
        } else {
            state.jobs += 1;
        }
        Ok(Guard {
            activity: self.clone(),
            exclusive,
        })
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        if let Ok(mut state) = self.activity.0.lock() {
            if self.exclusive {
                state.exclusive = false;
            } else {
                state.jobs -= 1;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jobs_and_installation_exclude_each_other_and_release_on_failure() {
        let activity = Activity::default();
        let job = activity.job().unwrap();
        assert!(activity.exclusive().is_err());
        drop(job);
        let install = activity.exclusive().unwrap();
        assert!(activity.job().is_err());
        assert!(activity.exclusive().is_err());
        drop(install);
        assert!(activity.job().is_ok());
    }
    #[test]
    fn all_jobs_must_finish() {
        let activity = Activity::default();
        let first = activity.job().unwrap();
        let second = activity.job().unwrap();
        drop(first);
        assert!(activity.exclusive().is_err());
        drop(second);
        assert!(activity.exclusive().is_ok());
    }
}
