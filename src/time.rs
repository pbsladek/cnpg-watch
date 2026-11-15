use crate::model::Tick;
use tokio::time::Instant;

/// Tokio time advances in scheduler tests; wall time never controls deadlines.
#[derive(Clone)]
pub struct Clock {
    origin: Instant,
}
impl Default for Clock {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}
impl Clock {
    pub fn now(&self) -> Tick {
        Tick(
            self.origin
                .elapsed()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
        )
    }
}
