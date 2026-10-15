use crate::{CoreError, JobSpec, JobStatus, Progress};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Default)]
pub struct Session {
    next_id: u64,
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
