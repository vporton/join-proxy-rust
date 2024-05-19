use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::{LockResult, TryLockResult};

// FIXME: use async mutex.

trait MutexGuard<'a, T: ?Sized + 'a>: Deref<Target = T> /*+ DerefMut<Target = T>*/ {
    fn set(&mut self, value: T);
}

impl<'a, T: ?Sized + 'a> MutexGuard<'a, T> for std::sync::MutexGuard<'a, T> {
    fn set(&mut self, value: T) {
        *self.deref_mut() = value;
    }
}

// TODO: more abstract error handling
trait Mutex<T: ?Sized> {
    // fn get_mut(&mut self) -> LockResult<&mut T>; // not available for Redis
    fn into_inner(self) -> LockResult<T>
        where
            T: Sized;
    fn lock(&self) -> LockResult<impl MutexGuard<'_, T>>;
    fn try_lock(&self) -> TryLockResult<impl MutexGuard<'_, T>>;
}

impl<T: ?Sized> Mutex<T> for std::sync::Mutex<T> {
    fn into_inner(self) -> LockResult<T>
        where
            T: Sized
    {
        self.into_inner()
    }

    fn lock(&self) -> LockResult<impl MutexGuard<'_, T>> {
        self.lock()
    }

    fn try_lock(&self) -> TryLockResult<impl MutexGuard<'_, T>> {
        self.try_lock()
    }
}

trait AbstractLockableMap<'a, K, V> {
    type Guard: MutexGuard<'a, Option<V>>;

    fn lock(&mut self, key: K) -> Self::Guard;

    // fn insert(&mut self, key: K, value: V);

    // fn get(&self, key: &K) -> Option<Self::Guard> {
    //     self.map.get(key).map(|mutex| mutex.lock().unwrap())
    // }
}

struct LockableMap<K, V> {
    map: HashMap<K, std::sync::Mutex<Option<V>>>,
}

impl<K, V> LockableMap<K, V> {
    pub fn new() -> Self {
        Self { map: HashMap::new() }
    }
}

// Code based on https://g.co/gemini/share/5045754c1381
impl<'a, K, V> AbstractLockableMap<'a, K, V> for LockableMap<K, V> 
where
    K: std::hash::Hash + Eq + Clone, // TODO: Is `Clone` needed?
    V: 'a
{
    type Guard = std::sync::MutexGuard<'a, Option<V>>;

    fn lock(&mut self, key: K) -> std::sync::MutexGuard<'a, Option<V>> {
        self.map.entry(key.clone())
            .or_insert_with(|| Mutex::new(None))
            .lock()
            .unwrap()
    }

    // fn insert(&mut self, key: K, value: V) {
    //     let mut lock = self.lock(key); // Lock the key first
    //     *lock = Some(value); // Then insert the value
    // }

    // fn get(&self, key: &K) -> Option<MutexGuard<'_, Option<V>>> {
    //     self.map.get(key).map(|mutex| mutex.lock().unwrap())
    // }
}
