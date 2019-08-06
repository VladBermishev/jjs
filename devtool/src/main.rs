mod util;

use std::fs;
use structopt::StructOpt;
use util::get_project_dir;

#[derive(StructOpt)]
struct TouchArgs {
    #[structopt(short = "v", long = "verbose")]
    verbose: bool,
}

#[derive(StructOpt)]
enum CliArgs {
    /// Helper command to setup VM with jjs
    #[structopt(name = "vm")]
    Vm,
    /// Touch all crates in workspace, so cargo-check or clippy will lint them
    #[structopt(name = "touch")]
    Touch(TouchArgs),
}

fn task_vm() {
    let addr = "0.0.0.0:4567";
    println!("address: {}", addr);
    let setup_script_path = format!("{}/devtool/scripts/vm-setup.sh", get_project_dir());
    let pkg_path = format!("{}/pkg/jjs.tgz", get_project_dir());
    let pg_start_script_path = format!("{}/devtool/scripts/postgres-start.sh", get_project_dir());
    rouille::start_server(addr, move |request| {
        let url = request.url();
        if url == "/setup" {
            return rouille::Response::from_file(
                "text/x-shellscript",
                fs::File::open(&setup_script_path).unwrap(),
            );
        } else if url == "/pkg" {
            return rouille::Response::from_file(
                "application/gzip",
                fs::File::open(&pkg_path).unwrap(),
            );
        } else if url == "/pg-start" {
            return rouille::Response::from_file(
                "text/x-shellscript",
                fs::File::open(&pg_start_script_path).unwrap(),
            );
        }

        rouille::Response::from_data("text/plain", "ERROR: NOT FOUND")
    });
}

fn task_touch(arg: TouchArgs) {
    let workspace_root = get_project_dir();
    let items = fs::read_dir(workspace_root).expect("couldn't list dir");
    //let mut roots = Vec::new();
    for item in items {
        let info = item.expect("couldn't describe item");
        let item_type = info.file_type().expect("couldn't get item type");
        if !item_type.is_dir() {
            continue;
        }
        let path = info
            .file_name()
            .to_str()
            .expect("couldn't decode item path")
            .to_owned();
        // TODO: touch bin/*
        for root in &["src/main.rs", "src/lib.rs", "build.rs"] {
            let p = format!("{}/{}", &path, root);
            if std::fs::metadata(&p).is_ok() {
                if arg.verbose {
                    println!("touching {}", &p);
                }
                let time = filetime::FileTime::from_system_time(std::time::SystemTime::now());
                filetime::set_file_times(&p, time, time).expect("couldn't touch");
            }
        }
    }
}

fn main() {
    let args = CliArgs::from_args();
    match args {
        CliArgs::Vm => task_vm(),
        CliArgs::Touch(arg) => task_touch(arg),
    }
}
