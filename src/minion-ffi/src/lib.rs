#![feature(try_trait)]

use minion;
use std::{
    collections::HashMap,
    ffi::{CStr, OsStr, OsString},
    mem,
    os::raw::c_char,
};

#[repr(i32)]
pub enum ErrorCode {
    /// operation completed successfully
    Ok,
    /// passed arguments didn't pass some basic checks
    /// examples:
    /// - provided buffer was expected to be null-terminated utf8-encoded string, but wasn't
    /// - something was expected to be unique, but wasn't, and so on
    /// these errors usually imply bug exists in caller code
    InvalidInput,
    /// unknown error
    Unknown,
}

unsafe fn get_string(buf: *const c_char) -> OsString {
    use std::os::unix::ffi::OsStrExt;
    let buf = CStr::from_ptr(buf);
    let buf = buf.to_bytes();
    let s = OsStr::from_bytes(buf);
    s.to_os_string()
}

impl std::ops::Try for ErrorCode {
    type Error = ErrorCode;
    type Ok = ErrorCode;

    fn into_result(self) -> Result<ErrorCode, ErrorCode> {
        match self {
            ErrorCode::Ok => Ok(ErrorCode::Ok),
            oth => Err(oth),
        }
    }

    fn from_error(x: ErrorCode) -> Self {
        x
    }

    fn from_ok(x: ErrorCode) -> Self {
        x
    }
}

pub struct Backend(Box<dyn minion::Backend>);

/// # Safety
/// Must be called once
/// Must be called before any library usage
#[no_mangle]
#[must_use]
pub unsafe extern "C" fn minion_lib_init() -> ErrorCode {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("[minion-ffi] PANIC: {} ({:?})", &info, info);
        std::process::abort();
    }));
    ErrorCode::Ok
}

/// Create backend, default for target platform
#[no_mangle]
#[must_use]
pub extern "C" fn minion_backend_create(out: &mut *mut Backend) -> ErrorCode {
    let backend = Backend(minion::setup());
    let backend = Box::new(backend);
    *out = Box::into_raw(backend);
    ErrorCode::Ok
}

/// Drop backend
/// # Safety
/// `b` must be pointer to Backend, allocated by `minion_backend_create`
#[no_mangle]
#[must_use]
pub unsafe extern "C" fn minion_backend_free(b: *mut Backend) -> ErrorCode {
    let b = Box::from_raw(b);
    mem::drop(b);
    ErrorCode::Ok
}

#[repr(C)]
pub struct TimeSpec {
    pub seconds: u32,
    pub nanoseconds: u32,
}

#[repr(C)]
pub struct DominionOptions {
    pub time_limit: TimeSpec,
    pub process_limit: u32,
    pub memory_limit: u32,
    pub isolation_root: *const c_char,
    pub shared_directories: *mut SharedDirectoryAccess,
}

#[derive(Clone)]
pub struct Dominion(minion::DominionRef);

/// # Safety
/// Provided arguments must be well-formed
#[no_mangle]
#[must_use]
pub unsafe extern "C" fn minion_dominion_create(
    backend: &Backend,
    options: DominionOptions,
    out: &mut *mut Dominion,
) -> ErrorCode {
    let mut exposed_paths = Vec::new();
    {
        let mut p = options.shared_directories;
        while !(*p).host_path.is_null() {
            let opt = minion::PathExpositionOptions {
                src: get_string((*p).host_path).into(),
                dest: get_string((*p).sandbox_path).into(),
                access: match (*p).kind {
                    SharedDirectoryAccessKind::Full => minion::DesiredAccess::Full,
                    SharedDirectoryAccessKind::Readonly => minion::DesiredAccess::Readonly,
                },
            };
            exposed_paths.push(opt);
            p = p.offset(1);
        }
    }
    let opts = minion::DominionOptions {
        max_alive_process_count: options.process_limit as _,
        memory_limit: u64::from(options.memory_limit),
        time_limit: std::time::Duration::new(
            options.time_limit.seconds.into(),
            options.time_limit.nanoseconds,
        ),
        isolation_root: get_string(options.isolation_root).into(),
        exposed_paths,
    };
    let d = backend.0.new_dominion(opts);
    let d = d.unwrap();

    let dw = Dominion(d);
    *out = Box::into_raw(Box::new(dw));
    ErrorCode::Ok
}

/// # Safety
/// `dominion` must be pointer, returned by `minion_dominion_create`.
#[no_mangle]
#[must_use]
pub unsafe extern "C" fn minion_dominion_free(dominion: *mut Dominion) -> ErrorCode {
    let b = Box::from_raw(dominion);
    mem::drop(b);
    ErrorCode::Ok
}

#[repr(C)]
pub struct EnvItem {
    pub name: *const c_char,
    pub value: *const c_char,
}

// minion-ffi will never modify nave or value, so no races can occur
unsafe impl Sync for EnvItem {}

#[no_mangle]
pub static ENV_ITEM_FIN: EnvItem = EnvItem {
    name: std::ptr::null(),
    value: std::ptr::null(),
};

#[repr(C)]
pub enum StdioMember {
    Stdin,
    Stdout,
    Stderr,
}

#[repr(C)]
pub struct StdioHandleSet {
    pub stdin: u64,
    pub stdout: u64,
    pub stderr: u64,
}

#[repr(C)]
pub struct ChildProcessOptions {
    pub image_path: *mut c_char,
    pub argv: *mut *mut c_char,
    pub envp: *mut EnvItem,
    pub stdio: StdioHandleSet,
    pub dominion: *mut Dominion,
    pub workdir: *mut c_char,
}

#[repr(C)]
pub enum SharedDirectoryAccessKind {
    Full,
    Readonly,
}

#[repr(C)]
pub struct SharedDirectoryAccess {
    pub kind: SharedDirectoryAccessKind,
    pub host_path: *const c_char,
    pub sandbox_path: *const c_char,
}

// minion-ffi will never modify host_path or sandbox_path, so no races can occur
unsafe impl Sync for SharedDirectoryAccess {}

#[no_mangle]
pub static SHARED_DIRECTORY_ACCESS_FIN: SharedDirectoryAccess = SharedDirectoryAccess {
    kind: SharedDirectoryAccessKind::Full, //doesn't matter
    host_path: std::ptr::null(),
    sandbox_path: std::ptr::null(),
};

pub struct ChildProcess(Box<dyn minion::ChildProcess>);

/// # Safety
/// Provided `options` must be well-formed
#[no_mangle]
#[must_use]
pub unsafe extern "C" fn minion_cp_spawn(
    backend: &Backend,
    options: ChildProcessOptions,
    out: &mut *mut ChildProcess,
) -> ErrorCode {
    let mut arguments = Vec::new();
    {
        let p = options.argv;
        while !(*p).is_null() {
            arguments.push(get_string(*p));
        }
    }
    let mut environment = HashMap::new();
    {
        let p = options.envp;
        while !(*p).name.is_null() {
            let name = get_string((*p).name);
            let value = get_string((*p).value);
            if environment.contains_key(&name) {
                return ErrorCode::InvalidInput;
            }
            environment.insert(name, value);
        }
    }
    let stdio = minion::StdioSpecification {
        stdin: minion::InputSpecification::handle(options.stdio.stdin),
        stdout: minion::OutputSpecification::handle(options.stdio.stdout),
        stderr: minion::OutputSpecification::handle(options.stdio.stderr),
    };
    let options = minion::ChildProcessOptions {
        path: get_string(options.image_path).into(),
        arguments,
        environment,
        dominion: (*options.dominion).0.clone(),
        stdio,
        pwd: get_string(options.workdir).into(),
    };
    let cp = backend.0.spawn(options).unwrap();
    let cp = ChildProcess(cp);
    let cp = Box::new(cp);
    *out = Box::into_raw(cp);
    ErrorCode::Ok
}
