use std::io;

use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
    System::{
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
        Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
        },
    },
};

/// A job object is kept for the complete lifetime of a CLI session. Closing it
/// after a force stop terminates every descendant, including shells/tools the
/// CLI spawned.
pub(crate) struct JobObject {
    handle: HANDLE,
}

// A Windows HANDLE is an owned kernel handle and may be moved between Tokio
// worker threads; all access remains behind &self and the handle is closed once.
unsafe impl Send for JobObject {}
unsafe impl Sync for JobObject {}

impl JobObject {
    pub(crate) fn for_pid(pid: u32) -> io::Result<Self> {
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let job = Self { handle };
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let set_result = unsafe {
            SetInformationJobObject(
                job.handle,
                JobObjectExtendedLimitInformation,
                (&mut limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if set_result == 0 {
            return Err(io::Error::last_os_error());
        }

        let process = unsafe {
            OpenProcess(
                PROCESS_SET_QUOTA | PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
                0,
                pid,
            )
        };
        if process.is_null() {
            return Err(io::Error::last_os_error());
        }
        let assigned = unsafe { AssignProcessToJobObject(job.handle, process) };
        unsafe { CloseHandle(process) };
        if assigned == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(job)
    }

    pub(crate) fn terminate(&self) -> io::Result<()> {
        let result = unsafe { TerminateJobObject(self.handle, 1) };
        if result == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

impl Drop for JobObject {
    fn drop(&mut self) {
        if !self.handle.is_null() && self.handle != INVALID_HANDLE_VALUE {
            unsafe { CloseHandle(self.handle) };
        }
    }
}
