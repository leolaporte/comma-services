use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::process::Command as AsyncCommand;
use tokio::time::timeout;

const CMD_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceScope {
    System,
    User,
}

#[derive(Debug, Clone)]
pub struct Service {
    pub name: String,
    pub enabled: bool,
    pub scope: ServiceScope,
}

pub fn list_services(scope: &ServiceScope) -> Result<Vec<Service>> {
    let mut cmd = Command::new("systemctl");
    if *scope == ServiceScope::User {
        cmd.arg("--user");
    }
    cmd.args(["list-unit-files", "--type=service", "--no-pager", "--no-legend"]);

    let output = cmd.output().context("Failed to run systemctl")?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    let services = stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let name = parts[0].to_string();
                let state = parts[1];
                // Only include services that can be manually enabled/disabled.
                // Skip static, generated, alias, transient, indirect, masked.
                let toggleable = matches!(
                    state,
                    "enabled" | "enabled-runtime" | "disabled" | "linked" | "linked-runtime"
                );
                if !toggleable {
                    return None;
                }
                let enabled = matches!(state, "enabled" | "enabled-runtime" | "linked");
                Some(Service {
                    name,
                    enabled,
                    scope: scope.clone(),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(services)
}

#[derive(Debug, Clone)]
pub enum ChangeAction {
    Enable,
    Disable,
}

#[derive(Debug, Clone)]
pub struct PendingChange {
    pub service: String,
    pub scope: ServiceScope,
    pub action: ChangeAction,
}

#[derive(Debug)]
pub struct ChangeResult {
    pub service: String,
    pub success: bool,
    pub message: String,
}

/// Apply changes using async commands with a timeout per command.
/// Separates enable/disable from start/stop so the enable always succeeds
/// even if the service is slow to start.
pub async fn apply_changes(changes: Vec<PendingChange>) -> Vec<ChangeResult> {
    let mut results = Vec::new();

    for change in &changes {
        let (enable_action, start_action) = match change.action {
            ChangeAction::Enable => ("enable", "start"),
            ChangeAction::Disable => ("disable", "stop"),
        };

        // Step 1: enable/disable (should be instant)
        let enable_result = run_systemctl(&change.scope, enable_action, &change.service).await;
        match enable_result {
            Ok(output) if output.status.success() => {
                // Step 2: start/stop (might be slow, use timeout)
                let start_result =
                    run_systemctl(&change.scope, start_action, &change.service).await;
                match start_result {
                    Ok(output) if output.status.success() => {
                        results.push(ChangeResult {
                            service: change.service.clone(),
                            success: true,
                            message: format!("{}d and {}ed", enable_action, start_action),
                        });
                    }
                    Ok(output) => {
                        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                        results.push(ChangeResult {
                            service: change.service.clone(),
                            success: false,
                            message: format!(
                                "{}d but {} failed: {}",
                                enable_action, start_action, stderr
                            ),
                        });
                    }
                    Err(e) => {
                        results.push(ChangeResult {
                            service: change.service.clone(),
                            success: false,
                            message: format!(
                                "{}d but {} timed out: {}",
                                enable_action, start_action, e
                            ),
                        });
                    }
                }
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                results.push(ChangeResult {
                    service: change.service.clone(),
                    success: false,
                    message: format!("{} failed: {}", enable_action, stderr),
                });
            }
            Err(e) => {
                results.push(ChangeResult {
                    service: change.service.clone(),
                    success: false,
                    message: format!("{} timed out: {}", enable_action, e),
                });
            }
        }
    }

    results
}

async fn run_systemctl(
    scope: &ServiceScope,
    action: &str,
    service: &str,
) -> Result<std::process::Output, String> {
    let mut cmd = match scope {
        ServiceScope::User => {
            let mut c = AsyncCommand::new("systemctl");
            c.args(["--user", action, service]);
            c
        }
        ServiceScope::System => {
            let mut c = AsyncCommand::new("pkexec");
            c.args(["systemctl", action, service]);
            c
        }
    };

    match timeout(CMD_TIMEOUT, cmd.output()).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(format!("command failed: {}", e)),
        Err(_) => {
            // Timeout — try to kill the child if possible
            Err("timed out after 10s".to_string())
        }
    }
}
