use chrono::{DateTime, Local};
use serde_json::Value;
use std::process::Command;

use crate::metrics::{Metric, MetricSource};

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";

/// Claude subscription plan usage, read from the same OAuth endpoint that
/// Claude Code's `/usage` command uses. Auth comes from the Claude Code
/// credentials already stored in the macOS Keychain, so this works as long
/// as you're logged in to Claude Code.
pub struct PlanUsage;

impl MetricSource for PlanUsage {
    fn name(&self) -> &str {
        "Claude Plan Usage"
    }

    fn fetch(&self) -> Result<Vec<Metric>, String> {
        let token = access_token()?;
        let body: Value = ureq::get(USAGE_URL)
            .set("Authorization", &format!("Bearer {token}"))
            .set("anthropic-beta", "oauth-2025-04-20")
            .call()
            .map_err(|e| match e {
                ureq::Error::Status(401, _) => {
                    "token rejected — run `claude` once to refresh login".to_string()
                }
                other => format!("usage request failed: {other}"),
            })?
            .into_json()
            .map_err(|e| format!("bad response: {e}"))?;

        let limits = body["limits"]
            .as_array()
            .ok_or("no `limits` array in usage response")?;

        let mut metrics: Vec<Metric> = limits
            .iter()
            .filter_map(|l| {
                let kind = l["kind"].as_str()?;
                let percent = l["percent"].as_f64()?;
                Some(Metric {
                    label: label_for(kind, l["scope"].as_str()),
                    percent,
                    resets_at: l["resets_at"]
                        .as_str()
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Local)),
                    emphasized: kind == "session",
                })
            })
            .collect();

        if metrics.is_empty() {
            return Err("usage response contained no limits".into());
        }
        // If the API ever stops reporting a session limit, emphasize the
        // highest metric so the title still shows something meaningful.
        if !metrics.iter().any(|m| m.emphasized) {
            if let Some(max) = metrics
                .iter_mut()
                .max_by(|a, b| a.percent.total_cmp(&b.percent))
            {
                max.emphasized = true;
            }
        }
        Ok(metrics)
    }
}

fn label_for(kind: &str, scope: Option<&str>) -> String {
    match (kind, scope) {
        ("session", _) => "Session (5h)".to_string(),
        ("weekly_all", _) => "Week · all models".to_string(),
        ("weekly_scoped", Some(scope)) => format!("Week · {scope}"),
        ("weekly_scoped", None) => "Week · scoped".to_string(),
        (other, Some(scope)) => format!("{other} · {scope}"),
        (other, None) => other.replace('_', " "),
    }
}

/// Read the Claude Code OAuth access token from the macOS Keychain via the
/// `security` CLI (which already has access, so no per-app keychain prompt).
fn access_token() -> Result<String, String> {
    let out = Command::new("security")
        .args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-w"])
        .output()
        .map_err(|e| format!("failed to run `security`: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "no Claude Code credentials in Keychain (service \"{KEYCHAIN_SERVICE}\") — log in with `claude` first"
        ));
    }
    let creds: Value = serde_json::from_slice(&out.stdout)
        .map_err(|e| format!("unexpected credential format: {e}"))?;
    creds["claudeAiOauth"]["accessToken"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "no accessToken in Keychain credentials".to_string())
}
