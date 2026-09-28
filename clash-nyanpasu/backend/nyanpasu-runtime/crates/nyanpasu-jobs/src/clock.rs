use jiff::Timestamp;
/// Wall-clock adapter. Interval deadlines use Tokio's monotonic clock instead.
pub trait Clock: Send + Sync + 'static {
    fn now(&self) -> Timestamp;
}
pub struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp::now()
    }
}
