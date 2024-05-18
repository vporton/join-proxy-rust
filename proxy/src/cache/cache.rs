use std::{ops::{Deref, DerefMut}, time::Duration};
use crate::errors::MyResult;

pub struct Key<'a>(pub &'a [u8]);
pub struct Value<'a>(pub &'a [u8]);

pub trait LockerGuard<'a, T: ?Sized + 'a>: Deref<Target = T> + DerefMut<Target = T> {}

pub trait Locker<T: ?Sized> {
    type LockError;
    type LockerGuard<'a>: LockerGuard<'a, T> where T: 'a, Self: 'a;
    fn lock(&self) -> Result<Self::LockerGuard<'_>, Self::LockError>;
}

pub trait Cache {
    type EntryLock: ?Sized;

    #[allow(unused)]
    fn keep_duration(&self) -> Duration;

    fn get(&mut self, key: &Key) -> MyResult<&Self::EntryLock>;

    fn put(&mut self, key: &Key, value: &Value) -> MyResult<()>;
}