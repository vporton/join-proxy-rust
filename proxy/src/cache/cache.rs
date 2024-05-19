use std::ops::Deref;
use std::fmt::Debug;
use std::{ops::{Deref, DerefMut}, time::Duration};
use lockable::TryInsertError;
// use lock_api::{MutexGuard, RawMutex};

use crate::errors::MyResult;

// pub struct Key<'a>(pub &'a [u8]);
// pub struct Value<'a>(pub &'a [u8]);

pub trait Guard<'a, V> {
    fn value(&self) -> Option<&V>;
    fn value_mut(&mut self) -> Option<&mut V>;
    fn remove(&mut self) -> Option<V>;
    fn insert(&mut self, value: V) -> Option<V>;
    fn try_insert(&mut self, value: V) -> Result<&mut V, TryInsertError<V>>;
    // fn value_or_insert_with(&mut self, value_fn: impl FnOnce() -> V) -> &mut V;
    fn value_or_insert(&mut self, value: V) -> &mut V;
}

impl<M, V, H, P> Guard<'_, V> for lockable::Guard<M, V, H, P> {
    fn value(&self) -> Option<&V> {
        Self::value()
    }
    fn value_mut(&mut self) -> Option<&mut V> {
        Self::value_mut()
    }
    fn remove(&mut self) -> Option<V> {
        Self::remove()
    }
    fn insert(&mut self, value: V) -> Option<V> {
        Self::insert(value)
    }
    fn try_insert(&mut self, value: V) -> Result<&mut V, TryInsertError<V>> {
        Self::try_insert(value)
    }
    fn value_or_insert(&mut self, value: V) -> &mut V {
        Self::value_or_insert(value)
    }
}

pub trait Cache<K, V> {
    type Guard<'a>
        where
            Self: 'a,
            K: 'a,
            V: 'a;

    fn get(&mut self, key: &K) -> MyResult<&dyn Guard<'_, V>>;

    fn put(&mut self, key: &K, value: &V) -> MyResult<()>;
}