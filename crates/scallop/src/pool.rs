use std::ffi::CString;
use std::fs::File;
use std::os::fd::AsFd;

use nix::errno::Errno;
use nix::unistd::{dup2_stderr, dup2_stdout};

use crate::Error;

/// Redirect stdout and stderr to a given raw file descriptor.
pub fn redirect_output<T: AsFd>(fd: T) -> crate::Result<()> {
    dup2_stdout(&fd).map_err(|e| Error::IO(e.to_string()))?;
    dup2_stderr(&fd).map_err(|e| Error::IO(e.to_string()))?;
    Ok(())
}

/// Suppress stdout and stderr.
pub fn suppress_output() -> crate::Result<()> {
    let f = File::options().write(true).open("/dev/null")?;
    redirect_output(&f)?;
    Ok(())
}

/// Semaphore wrapping libc named semaphore calls.
pub struct NamedSemaphore {
    sem: *mut libc::sem_t,
    size: u32,
}

impl NamedSemaphore {
    pub fn new<S: AsRef<str>>(name: S, size: usize) -> crate::Result<Self> {
        let name = CString::new(name.as_ref()).unwrap();
        let size: u32 = size
            .try_into()
            .map_err(|_| Error::Base(format!("pool too large: {size}")))?;

        let sem = unsafe { libc::sem_open(name.as_ptr(), libc::O_CREAT, 0o600, size) };
        if !sem.is_null() {
            unsafe { libc::sem_unlink(name.as_ptr()) };
            Ok(Self { sem, size })
        } else {
            let err = Errno::last_raw();
            Err(Error::Base(format!("sem_open() failed: {err}")))
        }
    }

    pub fn acquire(&mut self) -> crate::Result<()> {
        if unsafe { libc::sem_wait(self.sem) } == 0 {
            Ok(())
        } else {
            // grcov-excl-start: only errors on signal handler interrupt
            let err = Errno::last_raw();
            Err(Error::Base(format!("sem_wait() failed: {err}")))
        } // grcov-excl-stop
    }

    pub fn release(&mut self) -> crate::Result<()> {
        if unsafe { libc::sem_post(self.sem) } == 0 {
            Ok(())
        } else {
            let err = Errno::last_raw();
            Err(Error::Base(format!("sem_post() failed: {err}")))
        }
    }

    pub fn wait(&mut self) -> crate::Result<()> {
        for _ in 0..self.size {
            self.acquire()?;
        }
        Ok(())
    }
}

impl Drop for NamedSemaphore {
    fn drop(&mut self) {
        unsafe {
            libc::sem_close(self.sem);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semaphore() {
        // exceed max semaphore value
        let size = u32::MAX.try_into().unwrap();
        assert!(NamedSemaphore::new("test", size).is_err());

        // max value is i32::MAX
        let size = i32::MAX.try_into().unwrap();
        let mut sem = NamedSemaphore::new("test", size).unwrap();
        // overflow semaphore value
        assert!(sem.release().is_err());

        // acquire then release
        sem.acquire().unwrap();
        assert!(sem.release().is_ok());

        // acquire all
        let mut sem = NamedSemaphore::new("test", 10).unwrap();
        sem.wait().unwrap();
    }
}
