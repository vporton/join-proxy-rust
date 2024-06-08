use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

use async_trait::async_trait;

#[async_trait]
pub trait MutexGuard<T>: Deref<Target = T> /*+ DerefMut<Target = T>*/ {
    async fn set(&mut self, value: T);
    // fn remove(&self);
    // fn into_inner(self: Box<Self>) -> T where T: Sized + Clone; // TODO: `Clone` is bad.
    async fn inner(&self) -> T where T: Sized + Clone + std::marker::Sync; // TODO: `Clone` is bad.
}

#[async_trait]
impl<T> MutexGuard<T> for tokio::sync::MutexGuard<'_, T>
    where T: std::marker::Send
{
    async fn set(&mut self, value: T) {
        *self.deref_mut() = value;
    }

    // fn remove(&self) {
    //     *self.deref_mut() = None;
    // }

    async fn inner(&self) -> T where T: Sized + Clone + std::marker::Sync
    {
        (*self.deref()).clone()
    }
}

// TODO: more abstract error handling
// pub trait Mutex<T> {
//     // fn get_mut(&mut self) -> LockResult<&mut T>; // not available for Redis
//     // fn into_inner(self) -> T
//     //     where
//     //         T: Sized;
//     // async fn lock(&self) -> impl MutexGuard<T>;
//     // fn try_lock(&self) -> Result<impl MutexGuard<T>, TryLockError>;
// }

// impl<T> Mutex<T> for tokio::sync::Mutex<T>
//     where T: std::marker::Send,
// {
//     fn into_inner(self) -> T
//         where
//             T: Sized
//     {
//         self.into_inner()
//     }

//     async fn lock(&self) -> impl MutexGuard<T> {
//         self.lock().await
//     }

//     fn try_lock(&self) -> Result<impl MutexGuard<T>, TryLockError> {
//         self.try_lock()
//     }
// }

pub trait AbstractLockableMap<K, V> {
    type Guard<'a>: MutexGuard<Option<V>> where Self: 'a;

    async fn lock(&mut self, key: &K) -> Self::Guard<'_>;

    // fn insert(&mut self, key: K, value: V);

    // fn get(&self, key: &K) -> Option<Self::Guard> {
    //     self.map.get(key).map(|mutex| mutex.lock().unwrap())
    // }

    async fn remove(&mut self, key: &K);
}

pub struct LockableHashMap<K, V> {
    map: HashMap<K, tokio::sync::Mutex<Option<V>>>,
}

impl<K, V> LockableHashMap<K, V> {
    pub fn new() -> Self {
        Self { map: HashMap::new() }
    }
}

// Code based on https://g.co/gemini/share/5045754c1381
impl<K, V> AbstractLockableMap<K, V> for LockableHashMap<K, V> 
where
    K: std::hash::Hash + Eq + Clone, // TODO: Is `Clone` needed?
    V: std::marker::Send, // TODO: It is an over-specification.
{
    type Guard<'a> = tokio::sync::MutexGuard<'a, Option<V>> where K: 'a, V: 'a;

    async fn lock(&mut self, key: &K) -> tokio::sync::MutexGuard<Option<V>> {
        self.map.entry(key.clone())
            .or_insert_with(|| tokio::sync::Mutex::new(None))
            .lock()
            .await
    }

    async fn remove(&mut self, key: &K) {
        self.map.remove(key);
    }
}
