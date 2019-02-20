use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
};
use structopt::StructOpt;
#[derive(StructOpt)]
struct Options {
    /// Where to put files
    #[structopt(default_value = "/", long = "root", short = "r")]
    root: String,
    /// What toolchains to search
    #[structopt(long = "with")]
    with: Vec<String>,
}

#[derive(Clone, Copy)]
struct FindArgs<'a> {
    //TODO: root: &'a str,
    target: &'a str,
}

fn deduce_interpreter(path: &str) -> Option<String> {
    //TODO: check shebang
    let file_output = Command::new("file")
        .arg(path)
        .stderr(Stdio::inherit())
        .stdin(Stdio::null())
        .output()
        .expect("Couldn't describe");

    assert_eq!(file_output.status.success(), true);
    let info = String::from_utf8_lossy(&file_output.stdout).to_string();
    dbg!(&info);
    let info = info.split_whitespace();
    let info = info.map(|x| dbg!(x));
    let interp = info.skip_while(|t| *t != "interpreter").nth(1);
    interp.map(|s| s.to_string()).map(|s| s.replace(',', ""))
}

fn find_binary(args: FindArgs, bin_name: &str) {
    let full_path = Command::new("which")
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .stdout(Stdio::piped())
        .arg(bin_name)
        .output()
        .unwrap_or_else(|e| panic!("Couldn't resolve path to {}: error {}", bin_name, e));
    assert_eq!(full_path.status.success(), true);
    let full_path = String::from_utf8(full_path.stdout)
        .expect("Couldn't parse utf8")
        .trim()
        .to_string();
    let ldd_output = Command::new("ldd")
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .stdout(Stdio::piped())
        .arg(&full_path)
        .output()
        .unwrap_or_else(|e| panic!("Couldn't get dependencies of {}: error {}", full_path, e));
    assert_eq!(ldd_output.status.success(), true);
    let ldd_output = String::from_utf8(ldd_output.stdout).expect("Couldn't parse utf8");
    let deps = ldd_output
        .split('\n')
        .map(|x| dbg!(x))
        .filter_map(|line| line.split("=>").nth(1))
        .filter_map(|x| x.split_whitespace().nth(0))
        .map(|x| x.to_string());
    let interp = deduce_interpreter(full_path.as_str());
    let additional = [full_path];
    let items_to_copy = deps
        .chain(additional.iter().cloned())
        .chain(interp.iter().cloned());
    let mut options = fs_extra::dir::CopyOptions::new();
    options.skip_exist = true;
    for item in items_to_copy {
        let path = PathBuf::from(&item);
        let base_path = path.parent().unwrap().to_str().unwrap();
        let resulting_path = format!("{}{}", args.target, base_path);
        fs::create_dir_all(&resulting_path).ok();
        println!("Copying: {}", &item);
        fs_extra::copy_items(&vec![item], resulting_path, &options)
            .expect("Couldn't copy binary with its dependencies");
    }
}

fn main() {
    let opt: Options = Options::from_args();
    let arg = FindArgs { target: &opt.root };
    for bin in opt.with {
        find_binary(arg, bin.as_str());
    }
}
