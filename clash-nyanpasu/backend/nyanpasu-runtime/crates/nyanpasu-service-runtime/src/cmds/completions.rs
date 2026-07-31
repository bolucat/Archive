use clap::CommandFactory;
use clap_complete::aot::{Shell, generate};

use super::Cli;

/// Hidden on purpose: shell completion is for packagers and rc files, and the
/// GUI never calls it, so it does not belong in `--help`.
#[derive(Debug, clap::Args)]
pub struct CompletionsCommand {
    /// The shell to generate a completion script for
    #[arg(value_enum)]
    shell: Shell,
}

pub fn completions(ctx: CompletionsCommand) {
    generate(
        ctx.shell,
        &mut Cli::command(),
        crate::consts::APP_NAME,
        &mut std::io::stdout(),
    );
}

#[cfg(test)]
mod tests {
    use clap::ValueEnum;

    use super::*;

    #[test]
    fn every_shell_renders_a_completion_script() {
        for shell in Shell::value_variants() {
            let mut script = Vec::new();
            generate(
                *shell,
                &mut Cli::command(),
                crate::consts::APP_NAME,
                &mut script,
            );
            let script = String::from_utf8(script).expect("completion scripts are utf-8");
            assert!(
                script.contains(crate::consts::APP_NAME),
                "the {shell} script does not name the binary"
            );
        }
    }
}
