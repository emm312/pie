use std::{
    fs,
    io::Write,
    process::*, time::Instant,
};

use clap::{Parser, Subcommand};
use toml::*;
use anstyle::*;

#[derive(Parser, Debug)]
struct Args {
    #[command(subcommand)]
    subcommand: BuildCommand,
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

fn main() {
    let args = Args::parse();

    match args.subcommand {
        BuildCommand::New { path } => {
            std::fs::create_dir(&path).unwrap();
            std::fs::create_dir(format!("{}/src", path)).unwrap();
            std::fs::create_dir(format!("{}/include", path)).unwrap();

            let mut config = std::fs::File::create(format!("{}/config.toml", path)).unwrap();
            config
                .write(
                    format!(
                        r#"[package]
name = "{}"
deps = []
"#,
                        path
                    )
                    .as_bytes(),
                )
                .unwrap();

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
            let config = &std::fs::read_to_string("config.toml")
                .expect("Failed to find or read config file")
                .parse::<Table>()
                .expect("Invalid table.")["package"];
            let name = config.as_table().expect("invalid table")["name"]
                .as_str()
                .unwrap();
    
            build();
            std::process::Command::new(format!("./{}", name)).spawn().unwrap().wait().unwrap();
        }
        BuildCommand::Build => {
            build();
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

fn build() {
    // colorscheme
    let progress_good = Style::new().fg_color(Some(AnsiColor::Green.bright(true).into())).render();
    let progress_bad = Style::new().fg_color(Some(AnsiColor::Red.bright(true).into())).render();
    let progress_project = Style::new().fg_color(Some(AnsiColor::Yellow.bright(true).into())).italic().render();
    let reset = Reset.render();
    let color = ColorScheme {
        progress_good: &progress_good,
        progress_bad: &progress_bad,
        progress_project: &progress_project,
        reset: &reset,
    };

    let config = &std::fs::read_to_string("config.toml")
        .expect("Failed to find or read config file")
        .parse::<Table>()
        .expect("Invalid table.")["package"];
    let config = config.as_table().expect("invalid table");
    let name = config["name"]
        .as_str()
        .unwrap();
    let compiler_name = match config.get("compiler") {
        Some(v) => v.as_str().unwrap(),
        None => "g++-13"
    };

    println!("{}Building project {name}{}", color.progress_project, color.reset);
    let mut handles = Vec::new();
    let mut files = Vec::new();
    std::fs::create_dir("obj").unwrap_or(());
    let now = Instant::now();
    compile("src", config, &compiler_name, &color, &mut handles, &mut files);
    fn compile<'a>(dir: &str, config: &Table, compiler_name: &str, color: &ColorScheme<'a>, handles: &mut Vec<Child>, files: &mut Vec<String>) {
        std::fs::create_dir(format!("obj/{}", dir)).unwrap_or(());
        for file in fs::read_dir(dir).unwrap() {
            let file = file.unwrap();
            if file.file_type().unwrap().is_dir() {
                compile(
                    &format!("{}", file.path().display()),
                    config,
                    compiler_name,
                    color,
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
                for lib in config["deps"].as_array().expect("invalid deps arg") {
                    compiler.arg(format!("-l{}", lib.as_str().expect("invalid dep arg")));
                }

                handles.push(compiler.spawn().expect("failed to start compiler instance"));
                
                println!("{}compiling:{} {file_name}", color.progress_good, color.reset);
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

    let r = std::process::Command::new(compiler_name)
        .args(&files)
        .arg(format!("-o{}", name))
        .spawn()
        .expect("Failed to spawn linker process")
        .wait();
    if handle_err(r, &color) {
        exit(2);
    } else {
        println!("Finished compiling in {:.02}s", now.elapsed().as_secs_f64());
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
