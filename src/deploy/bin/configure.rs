use deploy::cfg;

use serde::ser::Serialize;
use std::{collections::HashMap, path::PathBuf};
use structopt::StructOpt;

#[derive(StructOpt)]
struct Opt {
    /// Do not build and install testlib
    #[structopt(long = "disable-testlib")]
    no_testlib: bool,
    /// Do not generate and install manual
    #[structopt(long = "disable-man")]
    no_man: bool,
    /// Do not build and install additional tools
    #[structopt(long = "disable-tools")]
    no_tools: bool,
    /// Do not build and install JJS core components
    #[structopt(long = "disable-core")]
    no_core: bool,
    /// Build and install extras (components, not directly related to JJS)
    #[structopt(long = "enable-extras")]
    extras: bool,
    /// Generate tarball
    #[structopt(long = "enable-archive")]
    archive: bool,
    /// Cargo path
    #[structopt(long, env = "CARGO")]
    cargo: Option<String>,
    /// CMake path
    #[structopt(long, env = "CMAKE")]
    cmake: Option<String>,
    /// Target triple
    #[structopt(long = "target", short = "T")]
    target: Option<String>,
    /// Optimization
    #[structopt(long = "optimize", short = "O")]
    optimize: bool,
    /// Debug symbols
    #[structopt(long = "dbg-dym", short = "D")]
    dbg_sym: bool,
    /// Emit verbose information about build
    #[structopt(long = "verbose", short = "V")]
    verbose: bool,
    /// Destination for artifacts
    #[structopt(long = "out", short = "P")]
    out_dir: Option<PathBuf>,
    /// Build deb packages
    #[structopt(long = "enable-deb")]
    deb: bool,
    /// Generate SystemD unit files
    #[structopt(long = "enable-systemd")]
    systemd: bool,
    /// Destination JJS will be installed to
    ///
    /// Some JJS components can not be built or work properly without this option
    /// By default, same as prefix.
    #[structopt(long)]
    install_prefix: Option<PathBuf>,
    /// Build docker images
    #[structopt(long = "enable-docker")]
    docker: bool,
    /// Docker image tag
    #[structopt(long)]
    docker_tag: Option<String>,
}

static MAKE_SCRIPT_TPL: &str = include_str!("../make-tpl.sh");
static MAKEFILE_TPL: &str = include_str!("../makefile.tpl");

fn generate_make_script(src: &str, build: &str) {
    let mut substitutions = HashMap::new();
    substitutions.insert("BUILD_DIR", build.to_string());
    substitutions.insert("SRC_DIR", src.to_string());
    let mut subst_text = String::new();
    for (k, v) in substitutions {
        let v_esc = shell_escape::escape(v.into());
        let line = format!("export JJS_{}=\"{}\"\n", k, &v_esc);
        subst_text.push_str(&line);
    }
    let script = MAKE_SCRIPT_TPL.replace("__SUBST__", &subst_text);
    let script_path = format!("{}/make", &build);
    std::fs::write(&script_path, script).unwrap();
    let full_script_path = std::fs::canonicalize(&script_path)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let deps_script_path = format!("{}/src/deploy/deps.sh", &src);
    let full_deps_script_path = std::fs::canonicalize(&deps_script_path)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let makefile = MAKEFILE_TPL
        .replace("___SCRIPT_PATH___", &full_script_path)
        .replace("___DEPS_SCRIPT_PATH___", &full_deps_script_path)
        .replace("    ", "\t");
    let makefile_path = format!("{}/Makefile", &build);
    std::fs::write(&makefile_path, makefile).unwrap();
}

fn check_build_dir(_src: &str, build: &str) {
    if deploy::util::create_or_empty(build).is_ok() {
        return;
    }
    let dot_build_file = format!("{}/.jjsbuild", build);
    if std::path::PathBuf::from(&dot_build_file).exists() {
        return;
    }
    eprintln!(
        "maybe, assumed build dir ({}) contains some important files. If you are sure, add .jjsbuild in this dir",
        build
    );
    std::process::exit(1);
}

fn check_env() {
    for (bin, cmd) in &[
        ("cmake", vec![]),
        ("gcc", vec!["--version"]),
        ("g++", vec!["--version"]),
        ("mdbook", vec!["--version"]),
    ] {
        print!("checking {} is installed... ", bin);
        let st = std::process::Command::new(bin)
            .args(cmd)
            .output()
            .map(|st| st.status.success())
            .unwrap_or(false);
        if !st {
            println!("no");
        } else {
            println!("ok");
        }
    }
}

fn main() {
    check_env();
    let jjs_path = std::env::var("JJS_CFGR_SOURCE_DIR").unwrap();
    let build_dir_path = std::env::current_dir()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    check_build_dir(&jjs_path, &build_dir_path);
    let opt: Opt = Opt::from_args();

    if let Some(tag) = &opt.docker_tag {
        if !tag.contains('%') {
            eprintln!("warning: --docker-tag is not tag template. Only last image will be tagged");
        }
    }

    let tool_info = cfg::ToolInfo {
        cargo: opt
            .cargo
            .as_ref()
            .map(String::as_str)
            .unwrap_or_else(|| "cargo")
            .to_string(),
        cmake: opt
            .cmake
            .as_ref()
            .map(String::as_str)
            .unwrap_or_else(|| "cmake")
            .to_string(),
    };
    let profile = match (opt.optimize, opt.dbg_sym) {
        (true, false) => cfg::BuildProfile::Release,
        (true, true) => cfg::BuildProfile::RelWithDebInfo,
        _ => cfg::BuildProfile::Debug,
    };
    let build_config = cfg::BuildConfig {
        target: match &opt.target {
            Some(t) => t.clone(),
            None => deploy::util::get_current_target(),
        },
        profile,
        tool_info,
    };
    let comps_config = cfg::ComponentsConfig {
        man: !opt.no_man,
        testlib: !opt.no_testlib,
        tools: !opt.no_tools,
        archive: opt.archive,
        core: !opt.no_core,
        extras: opt.extras,
    };
    let packaging = cfg::PackagingConfig {
        deb: opt.deb,
        systemd: opt.systemd,
        docker: opt.docker,
    };
    let config = cfg::Config {
        artifacts_dir: opt.out_dir.clone(),
        verbose: opt.verbose,
        packaging,
        build: build_config,
        components: comps_config,
        install_prefix: opt.install_prefix.clone().or_else(|| opt.out_dir.clone()),
        docker_tag: opt.docker_tag,
    };
    let manifest_path = format!("{}/jjs-build-config.json", &build_dir_path);
    println!("Emitting JJS build config: {}", &manifest_path);
    let out_file = std::fs::File::create(&manifest_path).unwrap();
    let mut ser = serde_json::Serializer::pretty(out_file);
    config.serialize(&mut ser).unwrap();
    generate_make_script(&jjs_path, &build_dir_path);
}
