use std::process::{Command, ExitCode};

use crate::config;
use crate::error::{OlaunchError, Result};
use crate::integrations::{self, LaunchPlan};

pub fn execute(plan: &LaunchPlan, configure_only: bool, dry_run: bool) -> Result<ExitCode> {
    if dry_run {
        print!("{}", plan.redacted_summary());
        return Ok(ExitCode::SUCCESS);
    }

    config::apply_edits(&plan.config_edits)?;
    if configure_only {
        return Ok(ExitCode::SUCCESS);
    }

    run_command(plan)
}

fn run_command(plan: &LaunchPlan) -> Result<ExitCode> {
    let mut command = Command::new(&plan.program);
    command.args(&plan.args);
    command.stdin(std::process::Stdio::inherit());
    command.stdout(std::process::Stdio::inherit());
    command.stderr(std::process::Stdio::inherit());
    for env in &plan.env {
        match &env.value {
            Some(value) => {
                command.env(&env.key, value);
            }
            None => {
                command.env_remove(&env.key);
            }
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        let err = command.exec();
        if err.kind() == std::io::ErrorKind::NotFound {
            return Err(missing_program_error(plan));
        }
        Err(OlaunchError::Io(err))
    }

    #[cfg(not(unix))]
    {
        let status = command.status().map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                missing_program_error(plan)
            } else {
                OlaunchError::Io(err)
            }
        })?;
        Ok(ExitCode::from(status.code().unwrap_or(1) as u8))
    }
}

fn missing_program_error(plan: &LaunchPlan) -> OlaunchError {
    let program = plan.program.display().to_string();
    let hint = integrations::get(&plan.integration)
        .map(|integration| integration.spec().install_hint.to_string())
        .unwrap_or_else(|_| "Install the missing program and ensure it is on PATH.".into());
    OlaunchError::MissingProgram { program, hint }
}
