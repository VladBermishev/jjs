pub mod check;
mod dominion;
mod jail_common;
mod jobserver;
mod pipe;
mod util;

pub use crate::linux::dominion::{DesiredAccess, LinuxDominion};
use crate::{
    linux::{
        pipe::{LinuxReadPipe, LinuxWritePipe},
        util::{err_exit, get_last_error, Handle, IgnoreExt, Pid},
    },
    Backend, ChildProcess, ChildProcessOptions, DominionOptions, DominionPointerOwner, DominionRef,
    InputSpecification, InputSpecificationData, OutputSpecification, OutputSpecificationData,
    WaitOutcome,
};
use nix::sys::memfd;
use snafu::ResultExt;
use std::{
    ffi::CString,
    fs,
    io::{Read, Write},
    os::unix::{ffi::OsStrExt, io::IntoRawFd},
    ptr,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc, Mutex,
    },
    time::{self, Duration},
};

pub type LinuxHandle = libc::c_int;

pub struct LinuxChildProcess {
    exit_code: AtomicI64,

    stdin: Option<Box<dyn Write + Send + Sync>>,
    stdout: Option<Box<dyn Read + Send + Sync>>,
    stderr: Option<Box<dyn Read + Send + Sync>>,
    //in order to save dominion while CP is alive
    _dominion_ref: DominionRef,

    pid: Pid,
}

const EXIT_CODE_STILL_RUNNING: i64 = i64::min_value();

// It doesn't intersect with normal exit codes
// because they fit in i32
impl ChildProcess for LinuxChildProcess {
    fn get_exit_code(&self) -> crate::Result<Option<i64>> {
        self.poll()?;
        let ec = self.exit_code.load(Ordering::SeqCst);
        let ec = match ec {
            EXIT_CODE_STILL_RUNNING => None,
            w => Some(w),
        };
        Ok(ec)
    }

    fn stdin(&mut self) -> Option<Box<dyn Write + Send + Sync>> {
        self.stdin.take()
    }

    fn stdout(&mut self) -> Option<Box<dyn Read + Send + Sync>> {
        self.stdout.take()
    }

    fn stderr(&mut self) -> Option<Box<dyn Read + Send + Sync>> {
        self.stderr.take()
    }

    fn wait_for_exit(&self, timeout: std::time::Duration) -> crate::Result<WaitOutcome> {
        if self.exit_code.load(Ordering::SeqCst) != EXIT_CODE_STILL_RUNNING {
            return Ok(WaitOutcome::AlreadyFinished);
        }
        let mut d = self._dominion_ref.d.lock().unwrap();
        let d = (*d).b.downcast_mut::<LinuxDominion>().unwrap();
        let wait_result = unsafe { d.poll_job(self.pid, timeout) };
        match wait_result {
            None => Ok(WaitOutcome::Timeout),
            Some(w) => {
                self.exit_code.store(i64::from(w), Ordering::SeqCst);
                Ok(WaitOutcome::Exited)
            }
        }
    }

    fn poll(&self) -> crate::Result<()> {
        self.wait_for_exit(Duration::from_nanos(1)).map(|_w| ())
    }

    fn is_finished(&self) -> crate::Result<bool> {
        self.poll()?;
        Ok(self.exit_code.load(Ordering::SeqCst) != EXIT_CODE_STILL_RUNNING)
    }

    fn kill(&mut self) -> crate::Result<()> {
        unsafe {
            if self.is_finished()? {
                return Ok(());
            }
            if libc::kill(self.pid, libc::SIGKILL) == -1 {
                err_exit("kill");
            }
            Ok(())
        }
    }
}

impl Drop for LinuxChildProcess {
    fn drop(&mut self) {
        let f = self.is_finished();
        if f.is_err() || !f.unwrap() {
            return;
        }
        self.kill().ignore();
        self.wait_for_exit(time::Duration::from_millis(100))
            .unwrap();
    }
}

fn handle_input_io(spec: InputSpecification) -> crate::Result<(Option<Handle>, Handle)> {
    match spec.0 {
        InputSpecificationData::Pipe => {
            let mut h_read = 0;
            let mut h_write = 0;
            pipe::setup_pipe(&mut h_read, &mut h_write)?;
            let f = unsafe { libc::dup(h_read) };
            unsafe { libc::close(h_read) };
            Ok((Some(h_write), f))
        }
        InputSpecificationData::Handle(rh) => {
            let h = rh as Handle;
            Ok((None, h))
        }
        InputSpecificationData::Empty => {
            let file = fs::File::create("/dev/null").context(crate::errors::Io)?;
            let file = file.into_raw_fd();
            Ok((None, file))
        }
        InputSpecificationData::Null => Ok((None, -1 as Handle)),
    }
}

fn handle_output_io(spec: OutputSpecification) -> crate::Result<(Option<Handle>, Handle)> {
    match spec.0 {
        OutputSpecificationData::Null => Ok((None, -1 as Handle)),
        OutputSpecificationData::Handle(rh) => Ok((None, rh as Handle)),
        OutputSpecificationData::Pipe => {
            let mut h_read = 0;
            let mut h_write = 0;
            pipe::setup_pipe(&mut h_read, &mut h_write)?;
            let f = unsafe { libc::dup(h_write) };
            unsafe { libc::close(h_write) };
            Ok((Some(h_read), f))
        }
        OutputSpecificationData::Ignore => {
            let file = fs::File::open("/dev/null").context(crate::errors::Io)?;
            let file = file.into_raw_fd();
            let fd = unsafe { libc::dup(file) };
            Ok((None, fd))
        }
        OutputSpecificationData::Buffer(sz) => {
            let memfd_name = "libminion_output_memfd";
            let memfd_name = CString::new(memfd_name).unwrap();
            let mut flags = memfd::MemFdCreateFlag::MFD_CLOEXEC;
            if sz.is_some() {
                flags |= memfd::MemFdCreateFlag::MFD_ALLOW_SEALING;
            }
            let mfd = memfd::memfd_create(&memfd_name, flags).unwrap();
            if let Some(sz) = sz {
                if unsafe { libc::ftruncate(mfd, sz as i64) } == -1 {
                    crate::errors::System {
                        code: get_last_error(),
                    }
                    .fail()?
                }
            }
            let child_fd = unsafe { libc::dup(mfd) };
            Ok((Some(mfd), child_fd))
        }
    }
}

fn spawn(options: ChildProcessOptions) -> crate::Result<LinuxChildProcess> {
    unsafe {
        let q = jail_common::JobQuery {
            image_path: options.path.clone(),
            argv: options.arguments.clone(),
            environment: options
                .environment
                .iter()
                .map(|(k, v)| (base64::encode(k.as_bytes()), v.clone()))
                .collect(),
            pwd: options.pwd.clone(),
        };

        let (in_w, in_r) = handle_input_io(options.stdio.stdin)?;
        let (out_r, out_w) = handle_output_io(options.stdio.stdout)?;
        let (err_r, err_w) = handle_output_io(options.stdio.stderr)?;

        let q = dominion::ExtendedJobQuery {
            job_query: q,

            stdin: in_r,
            stdout: out_w,
            stderr: err_w,
        };
        let mut d = options.dominion.d.lock().unwrap();
        let d = d.b.downcast_mut::<LinuxDominion>().unwrap();

        let spawn_result = d.spawn_job(q);

        // cleanup child stdio now
        libc::close(in_r);
        libc::close(out_w);
        libc::close(err_w);

        let ret = match spawn_result {
            Some(x) => x,
            None => return Err(crate::Error::Sandbox),
        };

        let mut stdin = None;
        if let Some(h) = in_w {
            let box_in: Box<dyn Write + Send + Sync> = Box::new(LinuxWritePipe::new(h));
            stdin.replace(box_in);
        }

        let process = |maybe_handle, out: &mut Option<Box<dyn Read + Send + Sync>>| {
            if let Some(h) = maybe_handle {
                let b: Box<dyn Read + Send + Sync> = Box::new(LinuxReadPipe::new(h));
                out.replace(b);
            }
        };

        let mut stdout = None;
        let mut stderr = None;

        process(out_r, &mut stdout);
        process(err_r, &mut stderr);

        Ok(LinuxChildProcess {
            exit_code: AtomicI64::new(EXIT_CODE_STILL_RUNNING),
            stdin,
            stdout,
            stderr,
            _dominion_ref: options.dominion.clone(),
            pid: ret.pid,
        })
    }
}

#[derive(Debug)]
pub struct LinuxBackend {}

impl Backend for LinuxBackend {
    fn new_dominion(&self, mut options: DominionOptions) -> crate::Result<DominionRef> {
        options.postprocess();
        let pd = Box::new(unsafe { LinuxDominion::create(options)? });
        Ok(DominionRef {
            d: Arc::new(Mutex::new(DominionPointerOwner { b: pd })),
        })
    }

    fn spawn(&self, options: ChildProcessOptions) -> crate::Result<Box<dyn ChildProcess>> {
        let cp = spawn(options);
        match cp {
            Ok(cp) => Ok(Box::new(cp)),
            Err(e) => Err(e),
        }
    }
}

fn empty_signal_handler(
    _signal_code: libc::c_int,
    _signal_info: *mut libc::siginfo_t,
    _ptr: *mut libc::c_void,
) {
}

fn fix_sigchild() {
    unsafe {
        let sa_ptr: *mut libc::sigaction = util::allocate_heap_variable();
        let mut sa = &mut *sa_ptr;
        sa.sa_sigaction = empty_signal_handler as *mut () as usize;
        libc::sigemptyset(&mut sa.sa_mask as *mut _);
        libc::sigaddset(&mut sa.sa_mask as *mut _, libc::SIGCHLD);
        sa.sa_flags = libc::SA_SIGINFO | libc::SA_RESTART;
        if libc::sigaction(libc::SIGCHLD, sa_ptr, ptr::null_mut()) == -1 {
            err_exit("sigaction");
        }
    }
}

pub fn setup_execution_manager() -> LinuxBackend {
    fix_sigchild();
    LinuxBackend {}
}
