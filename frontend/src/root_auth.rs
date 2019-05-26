// this module is responsible for root user authentification strategies
// it provides tcp service, which provides some platform-specific authentification options
use slog::{error, info, Logger};
use std::{
    mem,
    os::unix::{
        io::AsRawFd,
        net::{UnixListener, UnixStream},
    },
};
use std::sync::Arc;

#[derive(Clone)]
pub struct Config {
    pub socket_path: String,
    pub token_provider: Arc<dyn Fn() -> String + Send + Sync>,
}

fn handle_conn(logger: &Logger, cfg: &Config, mut conn: UnixStream) {
    use std::{ffi::c_void, io::Write};
    let conn_handle = conn.as_raw_fd();
    let mut peer_cred: libc::ucred = unsafe { mem::zeroed() };
    let mut len = mem::size_of_val(&peer_cred) as u32;
    unsafe {
        if libc::getsockopt(
            conn_handle,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut peer_cred as *mut _ as *mut c_void,
            &mut len,
        ) == -1
        {
            return;
        }
    }
    let my_uid = unsafe { libc::getuid() };
    if my_uid != peer_cred.uid {
        conn.write_all(b"error: your uid doesn't match that of jjs")
            .ok();
        return;
    }
    info!(logger, "issuing root credentials");
    let token = (cfg.token_provider)();
    let message = format!("==={}===\n", token);
    conn.write_all(message.as_bytes()).ok();
}

fn server_loop(logger: Logger, cfg: Config, sock: UnixListener) {
    info!(logger, "starting unix local root login service");
    for conn in sock.incoming() {
        if let Ok(conn) = conn {
            handle_conn(&logger, &cfg, conn)
        }
    }
}

pub fn start(logger: Logger, cfg: Config) {
    dbg!();
    info!(logger, "binding login server at {}", &cfg.socket_path);
    let listener = match UnixListener::bind(&cfg.socket_path) {
        Ok(l) => l,
        Err(err) => {
            error!(logger, "couldn't bind unix socket server due to {:?}",  err; "err" => ?err);
            return;
        }
    };
    std::thread::spawn(move || {
        server_loop(logger, cfg, listener);
    });
}