//! Health-check policy and transition tracking.

pub(crate) mod driver;
pub mod probe;

use std::{num::NonZeroU32, time::Duration};

use crate::{error::Error, probe::ProbeResult, spec::ResolvedController};

pub(crate) const MAX_LAST_ERROR_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthPolicy {
    interval: Duration,
    timeout: Duration,
    failure_threshold: NonZeroU32,
    success_threshold: NonZeroU32,
    start_period: Duration,
}

impl HealthPolicy {
    pub fn new(
        interval: Duration,
        timeout: Duration,
        failure_threshold: NonZeroU32,
        success_threshold: NonZeroU32,
        start_period: Duration,
    ) -> Result<Self, Error> {
        if interval.is_zero() {
            return Err(Error::InvalidHealthPolicy(
                "interval must be greater than zero".into(),
            ));
        }
        if timeout.is_zero() {
            return Err(Error::InvalidHealthPolicy(
                "timeout must be greater than zero".into(),
            ));
        }
        Ok(Self {
            interval,
            timeout,
            failure_threshold,
            success_threshold,
            start_period,
        })
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub fn failure_threshold(&self) -> NonZeroU32 {
        self.failure_threshold
    }

    pub fn success_threshold(&self) -> NonZeroU32 {
        self.success_threshold
    }

    pub fn start_period(&self) -> Duration {
        self.start_period
    }
}

impl Default for HealthPolicy {
    fn default() -> Self {
        Self {
            interval: Duration::from_millis(250),
            timeout: Duration::from_secs(1),
            failure_threshold: NonZeroU32::new(3).expect("non-zero"),
            success_threshold: NonZeroU32::MIN,
            start_period: Duration::ZERO,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrackerState {
    Starting,
    Healthy,
    Unhealthy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrackerUpdate {
    pub(crate) state: TrackerState,
    pub(crate) transitioned: bool,
    pub(crate) consecutive_failures: u32,
    pub(crate) last_error: Option<String>,
}

pub(crate) struct HealthTracker {
    policy: HealthPolicy,
    started_at: std::time::Instant,
    grace_ended: bool,
    state: TrackerState,
    consecutive_failures: u32,
    consecutive_successes: u32,
    last_error: Option<String>,
}

impl HealthTracker {
    pub(crate) fn new(policy: HealthPolicy, started_at: std::time::Instant) -> Self {
        Self {
            policy,
            started_at,
            grace_ended: false,
            state: TrackerState::Starting,
            consecutive_failures: 0,
            consecutive_successes: 0,
            last_error: None,
        }
    }

    pub(crate) fn observe(
        &mut self,
        now: std::time::Instant,
        result: &ProbeResult,
    ) -> TrackerUpdate {
        let previous = self.state;
        match result {
            ProbeResult::Healthy => {
                self.grace_ended = true;
                self.consecutive_failures = 0;
                self.last_error = None;
                if self.state == TrackerState::Healthy {
                    self.consecutive_successes = 0;
                } else {
                    self.consecutive_successes = self.consecutive_successes.saturating_add(1);
                    if self.consecutive_successes >= self.policy.success_threshold.get() {
                        self.state = TrackerState::Healthy;
                        self.consecutive_successes = 0;
                    }
                }
            }
            ProbeResult::Unhealthy { detail } => {
                self.consecutive_successes = 0;
                self.last_error = detail.as_deref().map(cap_detail);
                let grace_active =
                    !self.grace_ended && now < self.started_at + self.policy.start_period;
                if !grace_active {
                    self.consecutive_failures = self.consecutive_failures.saturating_add(1);
                    if self.state != TrackerState::Unhealthy
                        && self.consecutive_failures >= self.policy.failure_threshold.get()
                    {
                        self.state = TrackerState::Unhealthy;
                    }
                }
            }
        }
        TrackerUpdate {
            state: self.state,
            transitioned: previous != self.state,
            consecutive_failures: self.consecutive_failures,
            last_error: self.last_error.clone(),
        }
    }
}

fn cap_detail(detail: &str) -> String {
    if detail.len() <= MAX_LAST_ERROR_BYTES {
        return detail.to_owned();
    }
    let mut end = MAX_LAST_ERROR_BYTES;
    while !detail.is_char_boundary(end) {
        end -= 1;
    }
    detail[..end].to_owned()
}

pub(crate) fn build_control_client(
    controller: &ResolvedController,
    timeout: Duration,
) -> Result<clash_api::Client, Error> {
    let mut builder = clash_api::Client::builder(controller.host.clone())
        .configure_reqwest(|builder| builder.timeout(timeout));
    if let Some(secret) = &controller.secret {
        builder = builder.secret(secret.as_str());
    }
    Ok(builder.build()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU32;

    fn policy(failures: u32, successes: u32, start_period: Duration) -> HealthPolicy {
        HealthPolicy::new(
            Duration::from_millis(10),
            Duration::from_millis(20),
            NonZeroU32::new(failures).unwrap(),
            NonZeroU32::new(successes).unwrap(),
            start_period,
        )
        .unwrap()
    }

    #[test]
    fn policy_rejects_zero_interval_and_timeout() {
        assert!(
            HealthPolicy::new(
                Duration::ZERO,
                Duration::from_secs(1),
                NonZeroU32::MIN,
                NonZeroU32::MIN,
                Duration::ZERO,
            )
            .is_err()
        );
        assert!(
            HealthPolicy::new(
                Duration::from_secs(1),
                Duration::ZERO,
                NonZeroU32::MIN,
                NonZeroU32::MIN,
                Duration::ZERO,
            )
            .is_err()
        );
    }

    #[test]
    fn starting_success_threshold_and_interrupted_streak() {
        let start = std::time::Instant::now();
        let mut tracker = HealthTracker::new(policy(3, 2, Duration::ZERO), start);

        let first = tracker.observe(start, &ProbeResult::Healthy);
        assert_eq!(first.state, TrackerState::Starting);
        assert!(!first.transitioned);

        tracker.observe(
            start,
            &ProbeResult::Unhealthy {
                detail: Some("not yet".into()),
            },
        );
        let restarted = tracker.observe(start, &ProbeResult::Healthy);
        assert_eq!(restarted.state, TrackerState::Starting);

        let ready = tracker.observe(start, &ProbeResult::Healthy);
        assert_eq!(ready.state, TrackerState::Healthy);
        assert!(ready.transitioned);
    }

    #[test]
    fn failures_cross_threshold_without_flapping_early() {
        let start = std::time::Instant::now();
        let mut tracker = HealthTracker::new(policy(3, 1, Duration::ZERO), start);
        tracker.observe(start, &ProbeResult::Healthy);

        for count in 1..3 {
            let update = tracker.observe(
                start,
                &ProbeResult::Unhealthy {
                    detail: Some(format!("failure {count}")),
                },
            );
            assert_eq!(update.state, TrackerState::Healthy);
            assert!(!update.transitioned);
        }
        let unhealthy = tracker.observe(
            start,
            &ProbeResult::Unhealthy {
                detail: Some("threshold".into()),
            },
        );
        assert_eq!(unhealthy.state, TrackerState::Unhealthy);
        assert!(unhealthy.transitioned);

        let repeated = tracker.observe(
            start,
            &ProbeResult::Unhealthy {
                detail: Some("still failing".into()),
            },
        );
        assert_eq!(repeated.state, TrackerState::Unhealthy);
        assert!(!repeated.transitioned);
    }

    #[test]
    fn unhealthy_recovery_requires_consecutive_successes() {
        let start = std::time::Instant::now();
        let mut tracker = HealthTracker::new(policy(1, 2, Duration::ZERO), start);
        tracker.observe(start, &ProbeResult::Healthy);
        tracker.observe(
            start,
            &ProbeResult::Unhealthy {
                detail: Some("down".into()),
            },
        );

        let first = tracker.observe(start, &ProbeResult::Healthy);
        assert_eq!(first.state, TrackerState::Unhealthy);
        tracker.observe(
            start,
            &ProbeResult::Unhealthy {
                detail: Some("again".into()),
            },
        );
        assert_eq!(
            tracker.observe(start, &ProbeResult::Healthy).state,
            TrackerState::Unhealthy
        );
        let recovered = tracker.observe(start, &ProbeResult::Healthy);
        assert_eq!(recovered.state, TrackerState::Healthy);
        assert!(recovered.transitioned);
    }

    #[test]
    fn start_period_ignores_failures_and_first_success_ends_it() {
        let start = std::time::Instant::now();
        let mut tracker = HealthTracker::new(policy(1, 2, Duration::from_secs(10)), start);
        let within_grace = start + Duration::from_secs(1);
        let ignored = tracker.observe(
            within_grace,
            &ProbeResult::Unhealthy {
                detail: Some("ignored".into()),
            },
        );
        assert_eq!(ignored.state, TrackerState::Starting);
        assert_eq!(ignored.consecutive_failures, 0);

        tracker.observe(within_grace, &ProbeResult::Healthy);
        let counted = tracker.observe(
            within_grace,
            &ProbeResult::Unhealthy {
                detail: Some("counted".into()),
            },
        );
        assert_eq!(counted.state, TrackerState::Unhealthy);
        assert_eq!(counted.consecutive_failures, 1);
    }

    #[test]
    fn failure_counter_saturates_and_error_is_capped() {
        let start = std::time::Instant::now();
        let mut tracker = HealthTracker::new(policy(1, 1, Duration::ZERO), start);
        tracker.consecutive_failures = u32::MAX;
        let update = tracker.observe(
            start,
            &ProbeResult::Unhealthy {
                detail: Some("x".repeat(MAX_LAST_ERROR_BYTES * 2)),
            },
        );
        assert_eq!(update.consecutive_failures, u32::MAX);
        assert!(update.last_error.as_ref().unwrap().len() <= MAX_LAST_ERROR_BYTES);
    }
}
