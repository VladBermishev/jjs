#![feature(option_flattening)]

mod compiler;
mod inter_api;
mod invoke_context;
mod invoker;
mod judge;
mod judge_log;
mod os_util;
mod valuer;

use cfg_if::cfg_if;
use db::schema::InvocationRequest;
use invoker::{InvokeContext, Invoker};
use slog_scope::{debug, error};
use snafu::ResultExt;
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{self, atomic::AtomicBool, Arc},
};

pub(crate) mod err {
    pub type ErrorBox = Box<dyn std::error::Error + Send + Sync + 'static>;

    use snafu::{Backtrace, Snafu};
    use std::fmt::{self, Debug, Display, Formatter};

    pub struct StringError(pub String);

    impl Display for StringError {
        fn fmt(&self, f: &mut Formatter) -> fmt::Result {
            Display::fmt(&self.0, f)
        }
    }

    impl Debug for StringError {
        fn fmt(&self, f: &mut Formatter) -> fmt::Result {
            Debug::fmt(&self.0, f)
        }
    }

    impl std::error::Error for StringError {}

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub))]
    pub enum Error {
        Minion {
            source: minion::Error,
            backtrace: Backtrace,
        },
        Io {
            source: std::io::Error,
            backtrace: Backtrace,
        },
        Db {
            source: db::Error,
            backtrace: Backtrace,
        },
        /// Usually, these errors occur if system was given malformed configuration
        /// For example, if interpolation string is bad
        #[snafu(display("bad config: {}", inner))]
        BadConfig {
            backtrace: Backtrace,
            inner: Box<dyn std::error::Error + Send + Sync + 'static>,
        },
        #[snafu(display("error: {}", inner))]
        Other {
            backtrace: Backtrace,
            inner: ErrorBox,
        },
    }

    impl From<std::io::Error> for Error {
        fn from(x: std::io::Error) -> Error {
            Error::Io {
                source: x,
                backtrace: Backtrace::new(),
            }
        }
    }

    impl From<db::Error> for Error {
        fn from(source: db::Error) -> Error {
            Error::Db {
                source,
                backtrace: Backtrace::new(),
            }
        }
    }

    impl From<ErrorBox> for Error {
        fn from(x: ErrorBox) -> Error {
            Error::Other {
                backtrace: Backtrace::new(),
                inner: x,
            }
        }
    }
}

pub(crate) use err::{Error, StringError};
use std::{path::Path, process::exit};

/// Secondary information, used for various interpolations
#[derive(Debug)]
pub(crate) struct SubmissionProps {
    pub metadata: HashMap<String, String>,
    pub id: i32,
}

/// Submission information, sufficient for judging
#[derive(Debug)]
pub(crate) struct SubmissionInfo {
    pub toolchain_cfg: cfg::Toolchain,
    pub problem_cfg: cfg::Problem,
    pub problem_data: pom::Problem,
    /// Directory for general files (source, build, invlog)
    pub root_dir: PathBuf,
    pub props: SubmissionProps,
}

#[derive(Debug)]
/// All invoker-related data, that will be passed to Invoker
pub(crate) struct InvokeRequest {
    pub submission: SubmissionInfo,
    /// Temporary directory
    pub work_dir: tempfile::TempDir,
    pub id: i32,
}

cfg_if! {
if #[cfg(target_os="linux")] {
    fn check_system() -> bool {
        if let Some(err) = minion::linux_check_environment() {
            eprintln!("system configuration problem: {}", err);
            return false;
        }
        true
    }
} else {
    fn check_system() -> bool {
        true
    }
}
}

fn submission_set_judge_outcome(
    conn: &dyn db::DbConn,
    submission_id: i32,
    outcome: invoker::InvokeOutcome,
    request: &InvocationRequest,
) -> Result<(), Error> {
    let run_patch = db::schema::RunPatch {
        status_code: Some(outcome.status.code.to_string()),
        status_kind: Some(outcome.status.kind.to_string()),
        score: Some(outcome.score as i32),
        rejudge_id: Some(request.invoke_revision as i32),
    };

    conn.run_update(submission_id, run_patch)?;
    Ok(())
}

struct Server {
    config: cfg::Config,
    db_conn: Box<dyn db::DbConn>,
    backend: Box<dyn minion::Backend>,
}

impl Server {
    fn serve_forever(&self) {
        let should_run = sync::Arc::new(sync::atomic::AtomicBool::new(true));
        {
            let should_run = sync::Arc::clone(&should_run);
            ctrlc::set_handler(move || {
                should_run.store(false, sync::atomic::Ordering::SeqCst);
            })
            .unwrap();
        }
        //TODO: start multiple threads
        self.thread_loop(Arc::clone(&should_run));
    }

    fn try_get_task(&self) -> Option<InvocationRequest> {
        let res: Option<InvocationRequest> = self
            .db_conn
            .inv_req_pop() // TODO handle error
            .ok()
            .flatten();

        res
    }

    /// called by every thread
    fn thread_loop(&self, should_run: Arc<AtomicBool>) {
        loop {
            if !should_run.load(sync::atomic::Ordering::SeqCst) {
                break;
            }

            let inv_req = match self.try_get_task() {
                Some(s) => s,
                None => {
                    std::thread::sleep(std::time::Duration::from_millis(2000));
                    continue;
                }
            };
            let run_id = inv_req.run_id;
            match self.process_task(inv_req) {
                Ok(_) => {}
                Err(err) => {
                    error!("Invokation fault"; "submission" => run_id, "message" => %err, "message-detailed" => ?err);
                }
            }
        }
    }

    fn process_task(&self, inv_req: InvocationRequest) -> Result<(), Error> {
        let req = self.fetch_submission_info(&inv_req)?;
        let submission_id = req.submission.props.id;
        let outcome = self.process_invoke_request(&req);
        submission_set_judge_outcome(&*self.db_conn, submission_id, outcome, &inv_req)?;
        self.copy_invokation_data_dir_to_shared_fs(&req.work_dir.path(), submission_id, req.id)?;
        Ok(())
    }

    fn copy_invokation_data_dir_to_shared_fs(
        &self,
        temp_path: &Path,
        run_id: i32,
        inv_id: i32,
    ) -> Result<(), Error> {
        let target_dir = self
            .config
            .sysroot
            .join("var/submissions")
            .join(format!("s-{}", run_id))
            .join(format!("i-{}", inv_id));
        std::fs::create_dir_all(&target_dir)?;
        let from: Result<Vec<_>, _> = std::fs::read_dir(temp_path)?
            .map(|x| x.map(|y| y.path()))
            .collect();
        fs_extra::copy_items(&from?, &target_dir, &fs_extra::dir::CopyOptions::new())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        Ok(())
    }

    fn process_invoke_request(&self, request: &InvokeRequest) -> invoker::InvokeOutcome {
        use std::error::Error;
        let invoke_ctx = InvokeContext {
            minion_backend: &*self.backend,
            cfg: &self.config,
            problem_cfg: &request.submission.problem_cfg,
            toolchain_cfg: &request.submission.toolchain_cfg,
            problem_data: &request.submission.problem_data,
            submission_props: &request.submission.props,
        };
        let invoker = Invoker::new(invoke_ctx, request);
        debug!("Executing invoker request"; "request" => ?request, "submission" => ?request.submission.props.id, "workdir" => ?request.work_dir.path().display());
        let status = invoker.invoke().unwrap_or_else(|err| {
            let cause = err
                .source()
                .map(|e| e.to_string())
                .unwrap_or_else(|| "<missing>".to_string());
            let backtrace = snafu::ErrorCompat::backtrace(&err)
                .map(|bt| bt.to_string())
                .unwrap_or_else(|| "<not captured>".to_string());
            error!("Judge fault: {}", err; "backtrace" => backtrace, "cause" => cause);
            let st = invoker_api::Status {
                kind: invoker_api::StatusKind::InternalError,
                code: invoker_api::status_codes::JUDGE_FAULT.to_string(),
            };
            invoker::InvokeOutcome {
                status: st,
                score: 0,
            }
        });

        debug!("Judging finished"; "outcome" => ?status, "submission" => ?request.submission.props.id);
        status
    }

    /// This functions queries all related data about submission and returns JudgeRequest
    fn fetch_submission_info(
        &self,
        db_inv_req: &InvocationRequest,
    ) -> Result<InvokeRequest, Error> {
        let db_submission = self.db_conn.run_load(db_inv_req.run_id)?;

        let submission_root = self.config.sysroot.join("var/submissions");
        let submission_root = submission_root.join(&format!("s-{}", db_submission.id));

        let mut submission_metadata = HashMap::new();
        let judge_time = {
            let time = chrono::prelude::Utc::now();
            time.format("%Y-%m-%d %H:%M:%S").to_string()
        };
        submission_metadata.insert("JudgeTimeUtc".to_string(), judge_time);

        let prob_name = &db_submission.problem_id;

        let problem_manifest_path = self
            .config
            .sysroot
            .join("var/problems")
            .join(&prob_name)
            .join("manifest.json");

        let reader =
            std::io::BufReader::new(fs::File::open(problem_manifest_path).context(err::Io)?);

        let problem_data: pom::Problem =
            serde_json::from_reader(reader).map_err(|e| Error::Other {
                backtrace: Default::default(),
                inner: Box::new(e),
            })?;

        let toolchain_cfg = self
            .config
            .find_toolchain(&db_submission.toolchain_id)
            .ok_or(Error::BadConfig {
                backtrace: Default::default(),
                inner: Box::new(StringError(format!(
                    "toolchain {} not found",
                    &db_submission.toolchain_id
                ))),
            })?;

        let problem_cfg =
            self.config
                .find_problem(&db_submission.problem_id)
                .ok_or(Error::BadConfig {
                    backtrace: Default::default(),
                    inner: Box::new(StringError(format!(
                        "problem {} not found",
                        &db_submission.problem_id
                    ))),
                })?;

        let submission_props = SubmissionProps {
            metadata: submission_metadata,
            id: db_submission.id,
        };

        let submission = SubmissionInfo {
            root_dir: submission_root,
            props: submission_props,
            toolchain_cfg: toolchain_cfg.clone(),
            problem_data,
            problem_cfg: problem_cfg.clone(),
        };

        let req = InvokeRequest {
            submission,
            work_dir: tempfile::TempDir::new().context(err::Io {})?,
            id: db_inv_req.invoke_revision,
        };
        Ok(req)
    }
}

cfg_if! {
    if #[cfg(feature = "beautiful_backtrace")] {
        fn install_color_backtrace() {
            color_backtrace::install();
        }
    } else {
        fn install_color_backtrace() {

        }
    }
}

fn main() {
    dotenv::dotenv().ok();
    if atty::is(atty::Stream::Stderr) {
        install_color_backtrace();
    }
    util::log::setup();
    util::daemon_startup_sleep();
    util::wait::wait();

    let config = cfg::get_config();
    let db_conn = match db::connect_env() {
        Ok(db_conn) => db_conn,
        Err(e) => {
            eprintln!("Startup error: failed connect to database: {}", e);
            exit(1);
        }
    };

    if check_system() {
        debug!("system check passed")
    } else {
        return;
    }
    let backend = minion::setup();

    let invoker = Server {
        config,
        db_conn,
        backend,
    };

    util::daemon_notify_ready();

    invoker.serve_forever();
}
