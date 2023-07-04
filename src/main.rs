use std::{
    fs,
    io::Write,
    process::*, time::Instant,
};

use clap::{Parser, Subcommand};
use toml::*;
use anstyle::*;
use serde::{Serialize, Deserialize};

#[derive(Parser, Debug)]
struct Args {
    #[command(subcommand)]
    subcommand: BuildCommand,

    #[arg(long)]
    print_commands: bool,
}

#[derive(Subcommand, Clone, Debug)]
enum BuildCommand {
    Build,
    Run,
    New {
        #[arg()]
        path: String,
    },
}

#[derive(Serialize, Deserialize)]
struct Config {
    package: Package
}
#[derive(Serialize, Deserialize)]
struct Package {
    name: String,

    deps: Vec<String>,
    include_paths: Vec<String>,
    dep_paths: Vec<String>,

    compiler: Option<String>,
    flags: Vec<String>,
}

impl Config {
    fn new(name: &str) -> Self {
        Config {
            package: Package {
                name: name.to_string(),

                deps: vec![],
                include_paths: vec![],
                dep_paths: vec![],

                compiler: None,
                flags: vec![],
            }
        }
    }
}

fn main() {
    let args = Args::parse();

    match args.subcommand {
        BuildCommand::New { path } => {
            std::fs::create_dir(&path).unwrap();
            std::fs::create_dir(format!("{}/src", path)).unwrap();
            std::fs::create_dir(format!("{}/include", path)).unwrap();

            std::fs::write(format!("{}/config.toml", path), to_string(&Config::new(&path)).unwrap()).unwrap();

            let mut main = std::fs::File::create(format!("{}/src/main.cpp", path)).unwrap();
            main.write(
                r#"#include <iostream>
int main() {
    std::cout << "Hello, World!\n";
}"#
                .as_bytes(),
            )
            .unwrap();
        }
        BuildCommand::Run => {
            let config = from_str(&std::fs::read_to_string("config.toml").unwrap()).unwrap();
            build(&config, &args);
            std::process::Command::new(format!("./{}", config.package.name)).spawn().unwrap().wait().unwrap();
        }
        BuildCommand::Build => {
            let config = from_str(&std::fs::read_to_string("config.toml").unwrap()).unwrap();
            build(&config, &args);
        }
        // _ => todo!(),
    }
}

pub struct ColorScheme<'a> {
    pub progress_good: &'a dyn std::fmt::Display,
    pub progress_bad: &'a dyn std::fmt::Display,
    pub progress_project: &'a dyn std::fmt::Display,
    pub reset: &'a dyn std::fmt::Display,
}

fn build(config: &Config, args: &Args) {
    // colorscheme
    let progress_good = Style::new().fg_color(Some(AnsiColor::BrightGreen.into())).bold().render();
    let progress_bad = Style::new().fg_color(Some(AnsiColor::BrightRed.into())).bold().render();
    let progress_project = Style::new().fg_color(Some(AnsiColor::BrightYellow.into())).render();
    let reset = Reset.render();
    let color = ColorScheme {
        progress_good: &progress_good,
        progress_bad: &progress_bad,
        progress_project: &progress_project,
        reset: &reset,
    };

    let name = &config.package.name;
    let compiler_name = config.package.compiler.as_ref().map(|c| c.as_str()).unwrap_or("g++");

    println!("{}Building project {name}{}", color.progress_project, color.reset);
    let mut handles = Vec::new();
    let mut files = Vec::new();
    std::fs::create_dir("obj").unwrap_or(());
    let now = Instant::now();
    compile("src", config, &compiler_name, &color, args, &mut handles, &mut files);
    fn compile<'a>(dir: &str, config: &Config, compiler_name: &str, color: &ColorScheme<'a>, args: &Args, handles: &mut Vec<Child>, files: &mut Vec<String>) {
        std::fs::create_dir(format!("obj/{}", dir)).unwrap_or(());
        for file in fs::read_dir(dir).unwrap() {
            let file = file.unwrap();
            if file.file_type().unwrap().is_dir() {
                compile(
                    &format!("{}", file.path().display()),
                    config,
                    compiler_name,
                    color,
                    args,
                    handles,
                    files,
                );
            } else {
                let mut compiler = std::process::Command::new(compiler_name);
                compiler.arg("-c");
                let file_name = format!(
                    "./{}/{}",
                    dir,
                    file.file_name().into_string().unwrap(),
                );
                compiler.arg(&file_name);
                let obj_name = format!(
                    "./obj/{}.o",
                    file_name
                );
                compiler.arg(format!("-o{}", obj_name));

                files.push(obj_name);

                compiler.arg("-I./include");
                for inc_path in config.package.include_paths.iter() {
                    compiler.arg(format!("-I{}", inc_path));
                }

                for lib_path in config.package.dep_paths.iter() {
                    compiler.arg(format!("-L{}", lib_path));
                }

                for lib in config.package.deps.iter() {
                    compiler.arg(format!("-l{}", lib));
                }
                compiler.args(&config.package.flags);

                handles.push(compiler.spawn().expect("failed to start compiler instance"));
                
                println!("{}compiling:{} {file_name}", color.progress_good, color.reset);
                if args.print_commands { println!("{compiler:?}") }
            }
        }
    }

    let mut errors = 0;
    for mut handle in handles {
        if handle_err(handle.wait(), &color) {
            errors += 1;
        }
    }

    if errors != 0 {
        println!(
            "{}error:{} failed to compile project {name} due to {errors} error{}",
            color.progress_bad,
            color.reset,
            if errors != 1 { "s" } else { "" }
        );
        exit(1);
    }

    let mut linker = std::process::Command::new(compiler_name);
    linker.args(&files)
        .arg(format!("-o{}", name));

    linker.arg("-I./include");
    for inc_path in config.package.include_paths.iter() {
        linker.arg(format!("-I{}", inc_path));
    }

    for lib_path in config.package.dep_paths.iter() {
        linker.arg(format!("-L{}", lib_path));
    }

    for lib in config.package.deps.iter() {
        linker.arg(format!("-l{}", lib));
    }

    linker.args(&config.package.flags);

    println!("{}linking:{} {name}", color.progress_good, color.reset);
    if args.print_commands { println!("{linker:?}") }

    let r = linker.spawn()
        .expect("Failed to spawn linker process")
        .wait();

    if handle_err(r, &color) {
        exit(2);
    } else {
        println!("{}finished:{} {name} (in {:.02}s)", color.progress_good, color.reset, now.elapsed().as_secs_f64());
    }
}

fn handle_err<'a>(r: std::io::Result<ExitStatus>, color: &ColorScheme<'a>) -> bool {
    match r {
        Ok(ec) => {
            if !ec.success() {
                return true;
            }
        },
        Err(e) => {
            println!("{}error:{} {e:?}", color.progress_bad, color.reset);
            return true
        },
    }
    false
}
