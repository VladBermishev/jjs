use std::{
    env,
    ffi::{CStr, CString},
    fs,
    path::PathBuf,
    process::Command,
};
fn obtain_mount_list() -> Vec<PathBuf> {
    use std::str::FromStr;
    let mounts = Command::new("mount")
        .output()
        .expect("coudln't run `mount`")
        .stdout;
    let mounts = String::from_utf8_lossy(&mounts);
    mounts
        .split_whitespace()
        .map(|x| x.to_string())
        .filter(|it| it.starts_with("/"))
        .map(|x| PathBuf::from_str(&x).unwrap())
        .collect()
}

fn disintegrate(path: PathBuf, mounts: &[PathBuf]) {
    if path.is_file() {
        fs::remove_file(&path).expect("couldn't delete");
        return;
    }
    if mounts.contains(&path) {
        println!("unmounting {:?}", &path);
        let p = path.to_str().expect("ill-formed path");
        let p = CString::new(p).expect("ill-formed path");
        if libc::umount2(p.as_ptr(), libc::MNT_FORCE) == -1 {
            Err(std::io::Error::last_os_error()).unwrap();
        }
    }
    for item in fs::read_dir(&path).expect("coudln't list directory contents") {
        let item = item.expect("couldn't get item info");
        disintegrate(item.path(), mounts);
    }
}

fn main() {
    let path = env::args().nth(1).expect("Usage: disintegrate <path>");
    let mounts = obtain_mount_list();
    disintegrate(PathBuf::from_str(&path).unwrap(), &mounts);
}