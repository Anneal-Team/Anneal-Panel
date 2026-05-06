use std::{collections::HashMap, sync::RwLock};

use anneal_core::{ApplicationResult, QuotaState};
use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

use crate::domain::{QuotaDecision, QuotaEnvelope, UsageBatchItem, UsageSample};

#[async_trait]
pub trait UsageRepository: Send + Sync {
    async fn store_samples(&self, samples: Vec<UsageSample>) -> ApplicationResult<()>;
    async fn update_subscription_usage(
        &self,
        subscription_id: Uuid,
        used_bytes: i64,
        quota_state: QuotaState,
        suspended: bool,
    ) -> ApplicationResult<()>;
    async fn list_usage_overview(
        &self,
        tenant_id: Option<Uuid>,
    ) -> ApplicationResult<Vec<crate::domain::UsageOverview>>;
}

#[async_trait]
impl<T> UsageRepository for &T
where
    T: UsageRepository + Send + Sync,
{
    async fn store_samples(&self, samples: Vec<UsageSample>) -> ApplicationResult<()> {
        (*self).store_samples(samples).await
    }

    async fn update_subscription_usage(
        &self,
        subscription_id: Uuid,
        used_bytes: i64,
        quota_state: QuotaState,
        suspended: bool,
    ) -> ApplicationResult<()> {
        (*self)
            .update_subscription_usage(subscription_id, used_bytes, quota_state, suspended)
            .await
    }

    async fn list_usage_overview(
        &self,
        tenant_id: Option<Uuid>,
    ) -> ApplicationResult<Vec<crate::domain::UsageOverview>> {
        (*self).list_usage_overview(tenant_id).await
    }
}

pub struct UsageService<R> {
    repository: R,
}

impl<R> UsageService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
}

impl<R> UsageService<R>
where
    R: UsageRepository,
{
    pub async fn ingest(
        &self,
        samples: Vec<UsageBatchItem>,
        quotas: Vec<QuotaEnvelope>,
    ) -> ApplicationResult<HashMap<Uuid, QuotaDecision>> {
        let persisted = samples
            .iter()
            .map(|sample| UsageSample {
                id: Uuid::new_v4(),
                tenant_id: sample.tenant_id,
                subscription_id: sample.subscription_id,
                device_id: sample.device_id,
                bytes_in: sample.bytes_in,
                bytes_out: sample.bytes_out,
                measured_at: sample.measured_at,
                created_at: Utc::now(),
            })
            .collect::<Vec<_>>();
        self.repository.store_samples(persisted).await?;

        let mut deltas = HashMap::<Uuid, i64>::new();
        for sample in samples {
            let delta = sample.bytes_in + sample.bytes_out;
            deltas
                .entry(sample.subscription_id)
                .and_modify(|value| *value += delta)
                .or_insert(delta);
        }

        let mut decisions = HashMap::new();
        for quota in quotas {
            let used_bytes = quota.current_used_bytes
                + deltas
                    .get(&quota.subscription_id)
                    .copied()
                    .unwrap_or_default();
            let decision = decide(quota.traffic_limit_bytes, used_bytes);
            self.repository
                .update_subscription_usage(
                    quota.subscription_id,
                    decision.used_bytes,
                    decision.quota_state,
                    decision.suspend,
                )
                .await?;
            decisions.insert(quota.subscription_id, decision);
        }
        Ok(decisions)
    }

    pub async fn list_usage_overview(
        &self,
        tenant_id: Option<Uuid>,
    ) -> ApplicationResult<Vec<crate::domain::UsageOverview>> {
        self.repository.list_usage_overview(tenant_id).await
    }
}

#[derive(Default)]
pub struct InMemoryUsageRepository {
    samples: RwLock<Vec<UsageSample>>,
    states: RwLock<HashMap<Uuid, QuotaDecision>>,
}

#[async_trait]
impl UsageRepository for InMemoryUsageRepository {
    async fn store_samples(&self, samples: Vec<UsageSample>) -> ApplicationResult<()> {
        self.samples.write().expect("lock").extend(samples);
        Ok(())
    }

    async fn update_subscription_usage(
        &self,
        subscription_id: Uuid,
        used_bytes: i64,
        quota_state: QuotaState,
        suspended: bool,
    ) -> ApplicationResult<()> {
        self.states.write().expect("lock").insert(
            subscription_id,
            QuotaDecision {
                used_bytes,
                quota_state,
                suspend: suspended,
            },
        );
        Ok(())
    }

    async fn list_usage_overview(
        &self,
        _tenant_id: Option<Uuid>,
    ) -> ApplicationResult<Vec<crate::domain::UsageOverview>> {
        Ok(Vec::new())
    }
}

pub fn decide(quota_bytes: i64, used_bytes: i64) -> QuotaDecision {
    let ratio = if quota_bytes > 0 {
        used_bytes as f64 / quota_bytes as f64
    } else {
        1.0
    };
    if ratio >= 1.0 {
        QuotaDecision {
            used_bytes,
            quota_state: QuotaState::Exhausted,
            suspend: true,
        }
    } else if ratio >= 0.95 {
        QuotaDecision {
            used_bytes,
            quota_state: QuotaState::Warning95,
            suspend: false,
        }
    } else if ratio >= 0.80 {
        QuotaDecision {
            used_bytes,
            quota_state: QuotaState::Warning80,
            suspend: false,
        }
    } else {
        QuotaDecision {
            used_bytes,
            quota_state: QuotaState::Normal,
            suspend: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use uuid::Uuid;

    use anneal_core::QuotaState;

    use crate::{
        application::{InMemoryUsageRepository, UsageService, decide},
        domain::{QuotaEnvelope, UsageBatchItem},
    };

    #[test]
    fn hard_stop_on_limit() {
        let decision = decide(1_000, 1_000);
        assert_eq!(decision.quota_state, QuotaState::Exhausted);
        assert!(decision.suspend);
    }

    #[tokio::test]
    async fn ingest_updates_quota_state() {
        let service = UsageService::new(InMemoryUsageRepository::default());
        let subscription_id = Uuid::new_v4();
        let decisions = service
            .ingest(
                vec![UsageBatchItem {
                    tenant_id: Uuid::new_v4(),
                    subscription_id,
                    device_id: Uuid::new_v4(),
                    bytes_in: 500,
                    bytes_out: 450,
                    measured_at: Utc::now(),
                }],
                vec![QuotaEnvelope {
                    subscription_id,
                    traffic_limit_bytes: 1_000,
                    current_used_bytes: 0,
                }],
            )
            .await
            .expect("ingest");
        let decision = decisions.get(&subscription_id).expect("decision");
        assert_eq!(decision.quota_state, QuotaState::Warning95);
    }
}
