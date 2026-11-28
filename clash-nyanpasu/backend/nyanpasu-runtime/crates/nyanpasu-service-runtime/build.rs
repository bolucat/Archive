use jiff::Timestamp;
use rustc_version::version_meta;
use std::{env, process::Command};

fn main() {
    // Git Information
    let output = Command::new("git")
        .args([
            "show",
            "--pretty=format:'%H,%cn,%cI'",
            "--no-patch",
            "--no-notes",
        ])
        .output()
        .unwrap();
    let command_args: Vec<String> = String::from_utf8(output.stdout)
        .unwrap()
        .replace('\'', "")
        .split(',')
        .map(String::from)
        .collect();
    println!("cargo:rustc-env=COMMIT_HASH={}", command_args[0]);
    println!("cargo:rustc-env=COMMIT_AUTHOR={}", command_args[1]);
    let commit_date: Timestamp = command_args[2].parse().unwrap();
    println!("cargo:rustc-env=COMMIT_DATE={commit_date:.3}");

    // Build Date
    println!("cargo:rustc-env=BUILD_DATE={:.3}", Timestamp::now());

    // Build Profile
    println!(
        "cargo:rustc-env=BUILD_PROFILE={}",
        match env::var("PROFILE").unwrap().as_str() {
            "release" => "Release",
            "debug" => "Debug",
            _ => "Unknown",
        }
    );
    // Build Platform
    println!(
        "cargo:rustc-env=BUILD_PLATFORM={}",
        env::var("TARGET").unwrap()
    );
    // Rustc Version & LLVM Version
    let rustc_version = version_meta().unwrap();
    println!(
        "cargo:rustc-env=RUSTC_VERSION={}",
        rustc_version.short_version_string
    );
    println!(
        "cargo:rustc-env=LLVM_VERSION={}",
        match rustc_version.llvm_version {
            Some(v) => v.to_string(),
            None => "Unknown".to_string(),
        }
    )
}
