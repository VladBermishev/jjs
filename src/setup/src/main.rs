use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt)]
pub struct Opts {
    #[structopt(long)]
    data_dir: PathBuf,
    #[structopt(long)]
    install_dir: PathBuf,
    /// Connection string for database to setup
    ///
    /// If not provided, db setup will be skipped
    #[structopt(long)]
    db_url: Option<String>,
    #[structopt(long)]
    symlink_config: bool,
    #[structopt(long)]
    setup_config: bool,
    /// Build sample contest (requires ppc to be available)
    #[structopt(long)]
    sample_contest: bool,
    /// Force mode: ignore some errors
    #[structopt(long)]
    force: bool,
    /// Touch file on success
    #[structopt(long)]
    touch: Option<PathBuf>,
}

fn main() {
    let opts: Opts = Opts::from_args();
    util::log::setup();
    util::wait::wait();
    let params = setup::SetupParams {
        data_dir: opts.data_dir,
        install_dir: opts.install_dir,
        db: if let Some(uri) = opts.db_url {
            Some(setup::DatabaseParams { uri })
        } else {
            None
        },
        config: if opts.setup_config {
            Some(setup::ConfigParams {
                symlink: opts.symlink_config,
            })
        } else {
            None
        },
        sample_contest: opts.sample_contest,
        force: opts.force,
    };
    let runner = util::cmd::Runner::new();
    if let Err(e) = setup::setup(&params, &runner) {
        eprintln!("error: {}", e.source);
        eprintln!("at: {:?}", e.backtrace);
        std::process::exit(1);
    }
    runner.exit_if_errors();
    if let Some(touch) = &opts.touch {
        std::fs::File::create(touch).ok();
    }
}
