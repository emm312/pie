use std::{
    fs,
    io::Write,
    process::Child, time::Instant,
};

use clap::{Parser, Subcommand};
use toml::Table;

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
        _ => todo!(),
    }
}

fn build() {
    let config = &std::fs::read_to_string("config.toml")
        .expect("Failed to find or read config file")
        .parse::<Table>()
        .expect("Invalid table.")["package"];
    let name = config.as_table().expect("invalid table")["name"]
        .as_str()
        .unwrap();

    println!(
        "[Building project {}]",
        name
    );
    let mut handles = Vec::new();
    let mut files = Vec::new();
    std::fs::create_dir("obj").unwrap_or(());
    let now = Instant::now();
    compile("src", config.as_table().unwrap(), &mut handles, &mut files);
    fn compile(dir: &str, config: &Table, handles: &mut Vec<Child>, files: &mut Vec<String>) {
        std::fs::create_dir(format!("obj/{}", dir)).unwrap_or(());
        for file in fs::read_dir(dir).unwrap() {
            let file = file.unwrap();
            if file.file_type().unwrap().is_dir() {
                compile(
                    &format!("{}", file.path().display()),
                    config,
                    handles,
                    files,
                );
            } else {
                let mut compiler = std::process::Command::new("g++-13");
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
                handles.push(compiler.spawn().expect("failed to start g++ instance"));
            }
        }
    }
    for mut handle in handles {
        let _ = handle.wait(); // wait for all compilations to finish
    }

    std::process::Command::new("g++-13")
        .args(&files)
        .arg(format!("-o{}", name))
        .spawn()
        .expect("Failed to spawn linker process")
        .wait()
        .unwrap();
    println!("Finished compiling in {}s", now.elapsed().as_secs_f64());
}