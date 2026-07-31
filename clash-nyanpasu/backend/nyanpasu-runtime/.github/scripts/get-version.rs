#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2021"
[dependencies]
clap = { version = "4", features = ["derive"] }
toml = "0.8"
---

use clap::Parser;
use toml::Table;

#[derive(Parser, Debug)]
#[clap(version)]
struct Args {
    #[clap(short, long, help = "cargo file path")]
    path: std::path::PathBuf,
}

fn main() {
    let args = Args::parse();
    if !args.path.exists() {
        panic!("cargo file not found");
    }
    let content = std::fs::read_to_string(&args.path).unwrap();
    let table: Table = content.parse().unwrap();
    let version = table.get("package").unwrap().as_table().unwrap().get("version").unwrap();
    println!("{}", version.as_str().unwrap());
}
