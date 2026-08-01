pub mod direct;
pub mod lazy;
pub mod load_balance;
pub mod loopback;
pub mod resolve;
pub mod tunnel;

pub use lazy::LazyOutbound;
