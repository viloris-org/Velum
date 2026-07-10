use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const DURATION_MINIMUM_SAMPLES: usize = 20;
const FLAKE_MINIMUM_SAMPLES: usize = 100;
const REQUIRED_CI_P95_BUDGET_SECONDS: u64 = 900;
const FLAKE_RATE_BUDGET: f64 = 0.01;

#[derive(Debug, Deserialize)]
struct WorkflowRuns {
    workflow_runs: Vec<WorkflowRun>,
}

#[derive(Debug, Deserialize)]
struct WorkflowRun {
    conclusion: Option<String>,
    run_attempt: u32,
    run_started_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
struct Metrics {
    schema_version: u32,
    eligible_runs: usize,
    successful_reruns: usize,
    required_ci_p95_seconds: Option<u64>,
    flaky_run_rate: Option<f64>,
    duration_budget: BudgetStatus,
    flake_budget: BudgetStatus,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum BudgetStatus {
    InsufficientEvidence,
    Pass,
    Fail,
}

pub fn evaluate_files(input: &Path, output: &Path) -> Result<(), String> {
    let source = fs::read_to_string(input)
        .map_err(|error| format!("cannot read {}: {error}", input.display()))?;
    let runs: WorkflowRuns = serde_json::from_str(&source)
        .map_err(|error| format!("cannot parse {}: {error}", input.display()))?;
    let metrics = evaluate(&runs.workflow_runs)?;
    let serialized = serde_json::to_string_pretty(&metrics)
        .map_err(|error| format!("cannot serialize CI metrics: {error}"))?;
    fs::write(output, format!("{serialized}\n"))
        .map_err(|error| format!("cannot write {}: {error}", output.display()))?;
    println!(
        "CI health: {} eligible runs; duration {:?}; flakes {:?}.",
        metrics.eligible_runs, metrics.duration_budget, metrics.flake_budget
    );
    if metrics.duration_budget == BudgetStatus::Fail || metrics.flake_budget == BudgetStatus::Fail {
        Err("CI health budget exceeded; see retained metrics artifact".into())
    } else {
        Ok(())
    }
}

fn evaluate(runs: &[WorkflowRun]) -> Result<Metrics, String> {
    let eligible: Vec<_> = runs
        .iter()
        .filter(|run| {
            matches!(
                run.conclusion.as_deref(),
                Some("success" | "failure" | "timed_out")
            )
        })
        .collect();
    let mut durations = Vec::with_capacity(eligible.len());
    for run in &eligible {
        let started = OffsetDateTime::parse(&run.run_started_at, &Rfc3339)
            .map_err(|error| format!("invalid run_started_at: {error}"))?;
        let updated = OffsetDateTime::parse(&run.updated_at, &Rfc3339)
            .map_err(|error| format!("invalid updated_at: {error}"))?;
        let seconds = (updated - started).whole_seconds();
        if seconds < 0 {
            return Err("workflow run updated before it started".into());
        }
        durations.push(seconds as u64);
    }
    durations.sort_unstable();
    let p95 = percentile_95(&durations);
    let successful_reruns = eligible
        .iter()
        .filter(|run| run.conclusion.as_deref() == Some("success") && run.run_attempt > 1)
        .count();
    let flake_rate =
        (!eligible.is_empty()).then(|| successful_reruns as f64 / eligible.len() as f64);
    let duration_budget = if eligible.len() < DURATION_MINIMUM_SAMPLES {
        BudgetStatus::InsufficientEvidence
    } else if p95.is_some_and(|seconds| seconds <= REQUIRED_CI_P95_BUDGET_SECONDS) {
        BudgetStatus::Pass
    } else {
        BudgetStatus::Fail
    };
    let flake_budget = if eligible.len() < FLAKE_MINIMUM_SAMPLES {
        BudgetStatus::InsufficientEvidence
    } else if flake_rate.is_some_and(|rate| rate <= FLAKE_RATE_BUDGET) {
        BudgetStatus::Pass
    } else {
        BudgetStatus::Fail
    };

    Ok(Metrics {
        schema_version: 1,
        eligible_runs: eligible.len(),
        successful_reruns,
        required_ci_p95_seconds: p95,
        flaky_run_rate: flake_rate,
        duration_budget,
        flake_budget,
    })
}

fn percentile_95(sorted: &[u64]) -> Option<u64> {
    if sorted.is_empty() {
        return None;
    }
    let rank = (95 * sorted.len()).div_ceil(100);
    sorted.get(rank.saturating_sub(1)).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(seconds: u64, attempt: u32, conclusion: &str) -> WorkflowRun {
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        let seconds = seconds % 60;
        WorkflowRun {
            conclusion: Some(conclusion.into()),
            run_attempt: attempt,
            run_started_at: "2026-07-11T00:00:00Z".into(),
            updated_at: format!("2026-07-11T{hours:02}:{minutes:02}:{seconds:02}Z"),
        }
    }

    #[test]
    fn reports_insufficient_evidence_for_small_samples() {
        let metrics = evaluate(&[run(60, 1, "success")]).expect("metrics");
        assert_eq!(metrics.duration_budget, BudgetStatus::InsufficientEvidence);
        assert_eq!(metrics.flake_budget, BudgetStatus::InsufficientEvidence);
    }

    #[test]
    fn detects_duration_budget_failure() {
        let runs: Vec<_> = (0..20).map(|_| run(960, 1, "success")).collect();
        let metrics = evaluate(&runs).expect("metrics");
        assert_eq!(metrics.required_ci_p95_seconds, Some(960));
        assert_eq!(metrics.duration_budget, BudgetStatus::Fail);
    }

    #[test]
    fn counts_successful_reruns_as_flakes() {
        let mut runs: Vec<_> = (0..99).map(|_| run(60, 1, "success")).collect();
        runs.push(run(60, 2, "success"));
        let metrics = evaluate(&runs).expect("metrics");
        assert_eq!(metrics.successful_reruns, 1);
        assert_eq!(metrics.flaky_run_rate, Some(0.01));
        assert_eq!(metrics.flake_budget, BudgetStatus::Pass);
    }
}
