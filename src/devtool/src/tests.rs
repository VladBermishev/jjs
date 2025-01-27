use std::process::Command;
use structopt::StructOpt;
use util::cmd::{CommandExt, Runner};

#[derive(StructOpt)]
pub(crate) struct TestArgs {
    #[structopt(long)]
    verbose: bool,
    #[structopt(long, short = "i")]
    integration_tests: bool,
    #[structopt(long)]
    pub(crate) fail_fast: bool,
}

fn run_integ_test(runner: &Runner) {
    println!("Running integration tests");
    // TODO: hacky. Probably it can be done better.
    let out = Command::new("cargo")
        .current_dir("src/all")
        .args(&["test"])
        .arg("--")
        .arg("--list")
        .output()
        .expect("failed list integration tests")
        .stdout;

    let out = String::from_utf8(out).expect("cargo output is not utf8");
    let mut cnt_tests = 0;
    for line in out.lines() {
        if line.contains(": test") {
            let test_name = line
                .split_whitespace()
                .next()
                .expect("line is empty")
                .trim_end_matches(':');
            println!("Running: {}", test_name);
            Command::new("cargo")
                .current_dir("src/all")
                .args(&["test", test_name])
                .run_on(runner);
            cnt_tests += 1;
        }
    }
    println!("{} integration tests runned", cnt_tests);
}

pub(crate) fn task_test(args: TestArgs, runner: &Runner) {
    let mut cmd = Command::new("cargo");
    cmd.args(&["test"]);
    cmd.arg("--workspace");
    cmd.args(&["--exclude", "all"]);

    if args.verbose {
        cmd.args(&["--", "--nocapture"]);
    }
    if args.integration_tests {
        run_integ_test(runner);
    }
    cmd.run_on(runner);
}
