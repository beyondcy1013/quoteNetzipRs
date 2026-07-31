const MIN_REQUEST_INTERVAL_MS: u64 = 800;
const MIN_RESPONSE_INTERVAL_MS: u64 = 2_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenewalRequestItem {
    pub market: u8,
    pub code: String,
    pub token: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenewalSecurity {
    market: u8,
    code: String,
    last_request_ms: u64,
    last_response_ms: u64,
    token: u32,
}

impl RenewalSecurity {
    pub fn new(
        market: u8,
        code: impl Into<String>,
        last_request_ms: u64,
        last_response_ms: u64,
    ) -> Self {
        Self {
            market,
            code: code.into(),
            last_request_ms,
            last_response_ms,
            token: 0,
        }
    }

    fn eligible_at_ms(&self) -> u64 {
        self.last_request_ms
            .saturating_add(MIN_REQUEST_INTERVAL_MS)
            .max(
                self.last_response_ms
                    .saturating_add(MIN_RESPONSE_INTERVAL_MS),
            )
    }
}

#[derive(Clone, Debug, Default)]
pub struct QuoteRenewalScheduler {
    securities: Vec<RenewalSecurity>,
}

impl QuoteRenewalScheduler {
    pub fn new(securities: impl IntoIterator<Item = RenewalSecurity>) -> Self {
        Self {
            securities: securities.into_iter().collect(),
        }
    }

    pub fn take_due(&mut self, now_ms: u64, limit: usize) -> Vec<RenewalRequestItem> {
        let mut due = self
            .securities
            .iter()
            .enumerate()
            .filter_map(|(index, security)| {
                let eligible_at_ms = security.eligible_at_ms();
                (eligible_at_ms <= now_ms).then_some((
                    eligible_at_ms,
                    security.market,
                    security.code.clone(),
                    index,
                ))
            })
            .collect::<Vec<_>>();
        due.sort_unstable();

        due.into_iter()
            .take(limit)
            .map(|(_, _, _, index)| {
                let security = &mut self.securities[index];
                security.last_request_ms = now_ms;
                RenewalRequestItem {
                    market: security.market,
                    code: security.code.clone(),
                    token: security.token,
                }
            })
            .collect()
    }

    pub fn record_response(&mut self, market: u8, code: &str, token: u32, now_ms: u64) {
        if let Some(security) = self
            .securities
            .iter_mut()
            .find(|security| security.market == market && security.code == code)
        {
            security.token = token;
            security.last_response_ms = now_ms;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{QuoteRenewalScheduler, RenewalSecurity};

    #[test]
    fn renewal_requires_both_vendor_observed_time_gates() {
        let mut scheduler = QuoteRenewalScheduler::new([
            RenewalSecurity::new(0, "000001", 1_000, 1_000),
            RenewalSecurity::new(1, "600000", 0, 1_500),
            RenewalSecurity::new(1, "600001", 2_500, 0),
        ]);

        assert!(scheduler.take_due(1_799, 100).is_empty());
        assert_eq!(
            scheduler
                .take_due(3_000, 100)
                .into_iter()
                .map(|item| (item.market, item.code))
                .collect::<Vec<_>>(),
            vec![(0, "000001".to_string())]
        );
    }

    #[test]
    fn bounded_batches_do_not_starve_other_due_securities() {
        let mut scheduler = QuoteRenewalScheduler::new([
            RenewalSecurity::new(0, "000001", 0, 0),
            RenewalSecurity::new(1, "600000", 0, 0),
        ]);

        assert_eq!(scheduler.take_due(2_000, 1)[0].code, "000001");
        assert_eq!(scheduler.take_due(2_000, 1)[0].code, "600000");
    }

    #[test]
    fn response_rearms_security_with_latest_vendor_token() {
        let mut scheduler = QuoteRenewalScheduler::new([RenewalSecurity::new(
            1, "600000", 0, 0,
        )]);

        scheduler.record_response(1, "600000", 0x1234_5678, 1_000);

        assert!(scheduler.take_due(2_999, 100).is_empty());
        let due = scheduler.take_due(3_000, 100);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].token, 0x1234_5678);
    }
}
