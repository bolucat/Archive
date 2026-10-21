//! Select language and dispatch CLI or GUI startup (§1.2/§3.1), checking runtime only for GUI.

mod args;
mod cli;
mod i18n;
mod instance;
mod model;
mod netiso;
mod runtime_check;
// Shared with the release benchmark so it measures the production matcher.
#[path = "model/search.rs"]
pub mod search;
mod ui;

// rust-i18n's public macros resolve these hooks at the crate root; setup lives in i18n.
pub(crate) use i18n::{_rust_i18n_t, _rust_i18n_try_translate};
use windows::{
    Win32::{debugapi::OutputDebugStringW, winuser::*},
    core::HSTRING,
};

pub fn run() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let system = sys_locale::get_locale();
    let language = i18n::choose(args::language_hint(&arguments), system.as_deref());
    rust_i18n::set_locale(language);
    let parsed = match args::parse(&arguments) {
        Ok(parsed) => parsed,
        Err(error) => {
            let text = match error {
                args::Error::ParseName { .. } => rust_i18n::t!("cli.missing_name"),
                args::Error::ParseLanguage { .. } => rust_i18n::t!("cli.missing_language"),
            };
            show_message(&text, MB_ICONERROR as u32);
            std::process::exit(cli::FAILURE);
        }
    };
    let code = match parsed.mode {
        args::Mode::Cli(command) => cli::run(command),
        args::Mode::Gui(options) => run_gui(options),
    };
    std::process::exit(code);
}

fn run_gui(options: args::GuiOptions) -> i32 {
    // CLI and syntax have already returned; the original takes the GUI mutex before
    // constructing the window. Check runtime availability only after owning the instance.
    let primary = match instance::acquire() {
        Ok(instance::Outcome::Primary(primary)) => primary,
        Ok(instance::Outcome::Secondary { switched: true }) => return instance::SWITCHED,
        Ok(instance::Outcome::Secondary { switched: false }) => {
            show_message(
                &rust_i18n::t!("instance.already_running"),
                MB_ICONINFORMATION as u32,
            );
            return instance::CANNOT_SWITCH;
        }
        Err(error) => {
            show_message(
                &rust_i18n::t!("instance.start_failed", detail = error.to_string()),
                MB_ICONERROR as u32,
            );
            return instance::MUTEX_FAILURE;
        }
    };
    match runtime_check::ensure_available() {
        Ok(true) => match ui::run(options, primary.publisher()) {
            Ok(()) => cli::SUCCESS,
            Err(error) => {
                show_message(
                    &rust_i18n::t!("window.start_failed", detail = error.to_string()),
                    MB_ICONERROR as u32,
                );
                cli::FAILURE
            }
        },
        Ok(false) => cli::FAILURE,
        Err(error) => {
            debug(&error.to_string());
            cli::FAILURE
        }
    }
}

fn show_message(text: &str, flags: u32) {
    let text = HSTRING::from(text);
    let title = HSTRING::from(rust_i18n::t!("window.title").as_ref());
    // SAFETY: owned HSTRINGs remain live throughout this synchronous call.
    unsafe {
        MessageBoxW(None, &text, &title, flags);
    }
}

fn debug(text: &str) {
    let text = HSTRING::from(text);
    // SAFETY: text remains live for the synchronous debug-output call.
    unsafe {
        OutputDebugStringW(&text);
    }
}
