use serde::Deserialize;


#[derive(Debug, Deserialize)]
pub struct Period {
    pub start_timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SchedulerPlan {
    pub period: Period,
}