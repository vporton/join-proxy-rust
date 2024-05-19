use std::sync::{Mutex, MutexGuard};
use std::ops::Deref;
use std::fmt::Debug;
use std::{ops::{Deref, DerefMut}, time::Duration};
use crate::errors::MyResult;

pub struct Key<'a>(pub &'a [u8]);
pub struct Value<'a>(pub &'a [u8]);


pub trait Cache {
    type EntryLock: ?Sized;

    #[allow(unused)]
    fn keep_duration(&self) -> Duration;

    fn get(&mut self, key: &Key) -> MyResult<&Self::EntryLock>;

    fn put(&mut self, key: &Key, value: &Value) -> MyResult<()>;
}