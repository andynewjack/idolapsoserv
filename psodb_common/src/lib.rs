//! Database abstraction for managing persistent data, such as accounts and characters.

#[macro_use] extern crate log;
extern crate crypto;
extern crate rand;

pub mod error;
pub mod pool;

pub mod account;

pub use self::error::Error;
pub use self::account::Account;
pub use self::account::BbAccountInfo;
pub use self::pool::Pool;

use std::result;

/// Wrapper around the standard result that yields the database error type for Err.
pub type Result<T> = result::Result<T, Error>;

/// A backend implementation for the database.
///
/// When receiving a trait object on this trait, the implementing type should already have
/// initialized its resources so the methods would succeed under normal conditions.
pub trait Backend {
    /// Attempt to clone this Backend and create one that connects to the same database.
    /// A boxed Backend trait object is yielded so the trait doesn't have a Sized constraint.
    fn try_clone(&mut self) -> Result<Box<Backend>>;

    /// Retrieve an account by its ID.
    fn get_account_by_id(&self, id: u32) -> Result<Option<Account>>;

    /// Retrieve an account by its username.
    fn get_account_by_username(&self, username: &str) -> Result<Option<Account>>;

    /// Insert or update an account into the database, based on its internal ID value.
    fn put_account(&self, account: &mut Account) -> Result<()>;

    /// Reset or invalidate the passwords of every account.
    fn reset_account_passwords(&self) -> Result<()>;

    /// Get BB account information by account ID. If the BB account information doesn't exist,
    /// this should initialize the values to reasonable defaults.
    fn fetch_bb_account_info(&self, account_id: u32) -> Result<Option<BbAccountInfo>>;

    fn put_bb_account_info(&self, info: &BbAccountInfo) -> Result<()>;
}
