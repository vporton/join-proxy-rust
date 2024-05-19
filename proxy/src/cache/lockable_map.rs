use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::{LockResult, TryLockResult};

use tokio::sync::TryLockError;

// FIXME: use async mutex.

trait MutexGuard<T: ?Sized>: Deref<Target = T> /*+ DerefMut<Target = T>*/ {
    fn set(&mut self, value: T);
}

impl<T: ?Sized> MutexGuard<T> for tokio::sync::MutexGuard<'_, T> {
    fn set(&mut self, value: T) {
        *self.deref_mut() = value;
    }
}

// TODO: more abstract error handling
trait Mutex<T: ?Sized> {
    // fn get_mut(&mut self) -> LockResult<&mut T>; // not available for Redis
    fn into_inner(self) -> T
        where
            T: Sized;
    async fn lock(&self) -> impl MutexGuard<T>;
    fn try_lock(&self) -> Result<impl MutexGuard<T>, TryLockError>;
}

impl<T: ?Sized> Mutex<T> for tokio::sync::Mutex<T> {
    fn into_inner(self) -> T
        where
            T: Sized
    {
        self.into_inner()
    }

    async fn lock(&self) -> impl MutexGuard<T> {
        self.lock().await
    }

    fn try_lock(&self) -> Result<impl MutexGuard<T>, TryLockError> {
        self.try_lock()
    }
}

trait AbstractLockableMap<K, V> {
    type Guard<'a>: MutexGuard<Option<V>> where Self: 'a;

    async fn lock(&mut self, key: K) -> Self::Guard<'_>;

    // fn insert(&mut self, key: K, value: V);

    // fn get(&self, key: &K) -> Option<Self::Guard> {
    //     self.map.get(key).map(|mutex| mutex.lock().unwrap())
    // }
}

struct LockableMap<K, V> {
    map: HashMap<K, tokio::sync::Mutex<Option<V>>>,
}

impl<K, V> LockableMap<K, V> {
    pub fn new() -> Self {
        Self { map: HashMap::new() }
    }
}

// Code based on https://g.co/gemini/share/5045754c1381
impl<K, V> AbstractLockableMap<K, V> for LockableMap<K, V> 
where
    K: std::hash::Hash + Eq + Clone, // TODO: Is `Clone` needed?
{
    type Guard<'a> = tokio::sync::MutexGuard<'a, Option<V>> where K: 'a, V: 'a;

    async fn lock(&mut self, key: K) -> tokio::sync::MutexGuard<Option<V>> {
        self.map.entry(key.clone())
            .or_insert_with(|| tokio::sync::Mutex::new(None))
            .lock()
            .await
    }

    // fn insert(&mut self, key: K, value: V) {
    //     let mut lock = self.lock(key); // Lock the key first
    //     *lock = Some(value); // Then insert the value
    // }

    // fn get(&self, key: &K) -> Option<MutexGuard<'_, Option<V>>> {
    //     self.map.get(key).map(|mutex| mutex.lock().unwrap())
    // }
}
