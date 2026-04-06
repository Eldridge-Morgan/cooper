use cooper_codegen::analyzer::CronInfo;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Simple cron scheduler for local development.
/// Parses human-readable schedules ("every 1 hour") and cron expressions.
pub struct CronScheduler {
    jobs: Vec<CronJob>,
    running: Arc<RwLock<bool>>,
}

struct CronJob {
    info: CronInfo,
    interval_ms: u64,
}

impl CronScheduler {
    pub fn new(crons: &[CronInfo]) -> Self {
        let jobs = crons
            .iter()
            .map(|c| CronJob {
                info: c.clone(),
                interval_ms: parse_schedule(&c.schedule),
            })
            .collect();

        Self {
            jobs,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the cron scheduler in the background.
    pub fn start(&self, state: Arc<crate::router::AppState>) {
        for job in &self.jobs {
            let interval = job.interval_ms;
            let source = job.info.source_file.clone();
            let export = job.info.export_name.clone();
            let name = job.info.name.clone();
            let state = Arc::clone(&state);

            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(interval)).await;
                    tracing::info!("Running cron job: {}", name);

                    let t0 = std::time::Instant::now();
                    let rt = state.js_runtime.read().await;
                    let result = rt.call_cron(&source, &export).await;
                    let dur_ms = t0.elapsed().as_millis() as u64;

                    let status = match &result {
                        Ok(_) => "ok",
                        Err(_) => "error",
                    };

                    let _ = state.events_tx.send(
                        crate::router::DashboardEvent::now("cron", serde_json::json!({
                            "name": name,
                            "status": status,
                            "duration_ms": dur_ms,
                        }))
                    );

                    if let Err(e) = result {
                        tracing::error!("Cron job '{}' failed: {}", name, e);
                    }
                }
            });
        }
    }
}

/// Parse schedule strings into millisecond intervals.
/// Supports: "every N (second|minute|hour|day)" and standard cron expressions.
fn parse_schedule(schedule: &str) -> u64 {
    // Human-readable: "every 1 hour", "every 30 minutes", etc.
    if schedule.starts_with("every ") {
        let parts: Vec<&str> = schedule.trim_start_matches("every ").split_whitespace().collect();
        if parts.len() >= 2 {
            let n: u64 = parts[0].parse().unwrap_or(1);
            let unit = parts[1].trim_end_matches('s');
            return match unit {
                "second" => n * 1000,
                "minute" => n * 60_000,
                "hour" => n * 3_600_000,
                "day" => n * 86_400_000,
                _ => 3_600_000,
            };
        }
    }

    // Standard cron expression — parse minimally
    // For a full cron parser, we'd use the `cron` crate
    let parts: Vec<&str> = schedule.split_whitespace().collect();
    if parts.len() == 5 {
        // Check for simple patterns
        if let Ok(_minutes) = parts[0].parse::<u64>() {
            if parts[1] == "*" {
                // "N * * * *" — every hour at minute N
                return 3_600_000;
            }
        }
        // Default: treat as hourly
        return 3_600_000;
    }

    // Default: 1 hour
    3_600_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_schedule() {
        assert_eq!(parse_schedule("every 1 hour"), 3_600_000);
        assert_eq!(parse_schedule("every 30 minutes"), 1_800_000);
        assert_eq!(parse_schedule("every 5 seconds"), 5_000);
        assert_eq!(parse_schedule("every 1 day"), 86_400_000);
    }
}
