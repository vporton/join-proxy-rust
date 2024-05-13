use std::time::Duration;
use crate::errors::MyResult;

pub struct Key<'a>(pub &'a [u8]);
pub struct Value<'a>(pub &'a [u8]);

pub trait Cache {
    fn put(&mut self, key: Key, value: Value, save_for: Duration) -> MyResult<()>;
    fn get(&mut self, key: Key, save_for: Duration) -> MyResult<Option<&Vec<u8>>>;
}