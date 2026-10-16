use crate::{CoreError, JobSpec, JobStatus, Progress};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub type JobId = u64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub spec: JobSpec,
    pub status: JobStatus,
}

impl Job {
    pub fn new(id: JobId, spec: JobSpec) -> Self {
        Self {
            id,
            spec,
            status: JobStatus::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub jobs: Vec<Job>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Session {
    #[serde(default)]
    next_id: u64,
    #[serde(default)]
    jobs: Vec<Job>,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_job(&mut self, spec: JobSpec) -> JobId {
        self.next_id += 1;
        let id = self.next_id;
        self.jobs.push(Job::new(id, spec));
        id
    }

    pub fn job(&self, id: JobId) -> Option<&Job> {
        self.jobs.iter().find(|j| j.id == id)
    }

    pub fn job_mut(&mut self, id: JobId) -> Option<&mut Job> {
        self.jobs.iter_mut().find(|j| j.id == id)
    }

    pub fn jobs(&self) -> &[Job] {
        &self.jobs
    }

    pub fn remove_job(&mut self, id: JobId) -> Result<(), CoreError> {
        let before = self.jobs.len();
        self.jobs.retain(|j| j.id != id);
        if self.jobs.len() == before {
            return Err(CoreError::JobNotFound(id));
        }
        Ok(())
    }

    pub fn update_status(&mut self, id: JobId, status: JobStatus) -> Result<(), CoreError> {
        let job = self.job_mut(id).ok_or(CoreError::JobNotFound(id))?;
        job.status = status;
        Ok(())
    }

    pub fn update_progress(&mut self, id: JobId, progress: Progress) -> Result<(), CoreError> {
        let job = self.job_mut(id).ok_or(CoreError::JobNotFound(id))?;
        job.status.progress = progress;
        Ok(())
    }

    pub fn cancel_job(&mut self, id: JobId) -> Result<(), CoreError> {
        let job = self.job_mut(id).ok_or(CoreError::JobNotFound(id))?;
        job.status.state = crate::JobState::Cancelled;
        Ok(())
    }
    pub fn cancel(&mut self, id: JobId) -> Result<(), CoreError> {
        self.cancel_job(id)
    }
    pub fn mark_cancelled(&mut self, id: JobId) -> Result<(), CoreError> {
        self.cancel_job(id)
    }
    pub fn retry_job(&mut self, id: JobId) -> Result<(), CoreError> {
        let job = self.job_mut(id).ok_or(CoreError::JobNotFound(id))?;
        job.status = JobStatus::default();
        Ok(())
    }
    pub fn retry(&mut self, id: JobId) -> Result<(), CoreError> {
        self.retry_job(id)
    }
    pub fn enqueue(&mut self, spec: JobSpec) -> JobId {
        self.add_job(spec)
    }
    pub fn add(&mut self, spec: JobSpec) -> JobId {
        self.add_job(spec)
    }
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), CoreError> {
        let data =
            serde_json::to_string_pretty(self).map_err(|e| CoreError::Parse(e.to_string()))?;
        fs::write(path, data).map_err(|e| CoreError::Io(e.to_string()))
    }
    pub fn persist(&self, path: impl AsRef<Path>) -> Result<(), CoreError> {
        self.save(path)
    }
    pub fn save_to(&self, path: impl AsRef<Path>) -> Result<(), CoreError> {
        self.save(path)
    }
    pub fn write(&self, path: impl AsRef<Path>) -> Result<(), CoreError> {
        self.save(path)
    }
    pub fn to_file(&self, path: impl AsRef<Path>) -> Result<(), CoreError> {
        self.save(path)
    }
    pub fn export(&self, path: impl AsRef<Path>) -> Result<(), CoreError> {
        self.save(path)
    }
    pub fn load(path: impl AsRef<Path>) -> Result<Self, CoreError> {
        let data = fs::read_to_string(path).map_err(|e| CoreError::Io(e.to_string()))?;
        serde_json::from_str(&data).map_err(|e| CoreError::Parse(e.to_string()))
    }
    pub fn load_from(path: impl AsRef<Path>) -> Result<Self, CoreError> {
        Self::load(path)
    }
    pub fn restore(path: impl AsRef<Path>) -> Result<Self, CoreError> {
        Self::load(path)
    }
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, CoreError> {
        Self::load(path)
    }
    pub fn from_snapshot(snap: SessionSnapshot) -> Self {
        let mut s = Self::default();
        let mut next = 0;
        for j in snap.jobs {
            if j.id > next {
                next = j.id;
            }
            s.jobs.push(j);
        }
        s.next_id = next;
        s
    }
    pub fn snapshot_roundtrip(snap: SessionSnapshot) -> Self {
        Self::from_snapshot(snap)
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            jobs: self.jobs.clone(),
        }
    }

    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_add_and_read() {
        let mut s = Session::new();
        let id = s.add_job(JobSpec::default());
        assert_eq!(s.len(), 1);
        let job = s.job(id).unwrap();
        assert_eq!(job.status.state, crate::JobState::Queued);
        assert_eq!(s.jobs().first().unwrap().id, id);
    }

    #[test]
    fn session_remove_missing_errors() {
        let mut s = Session::new();
        assert_eq!(s.remove_job(999), Err(CoreError::JobNotFound(999)));
    }

    #[test]
    fn session_update_status() {
        let mut s = Session::new();
        let id = s.add_job(JobSpec::default());
        s.update_status(
            id,
            JobStatus {
                state: crate::JobState::Running,
                progress: Progress::new(0.25),
                message: Some("encoding".into()),
            },
        )
        .unwrap();
        assert_eq!(s.job(id).unwrap().status.state, crate::JobState::Running);
        assert_eq!(s.job(id).unwrap().status.progress.fraction, 0.25);
    }

    #[test]
    fn session_snapshot_roundtrip() {
        let mut s = Session::new();
        s.add_job(JobSpec::default());
        let snap = s.snapshot();
        assert_eq!(snap.jobs.len(), 1);
    }
}
