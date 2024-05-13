use std::time::Duration;
use crate::errors::MyResult;

pub struct Key<'a>(pub &'a [u8]);
pub struct Value<'a>(pub &'a [u8]);

pub trait Cache {
    #[allow(unused)]
    fn keep_duration(&self) -> Duration;

    fn put(&mut self, key: Key, value: Value) -> MyResult<()>;

    fn get(&mut self, key: Key) -> MyResult<Option<&Vec<u8>>>;
}