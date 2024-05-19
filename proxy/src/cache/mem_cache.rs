use std::{collections::HashMap, ops::{Deref, DerefMut}, sync::{Mutex, MutexGuard, PoisonError}, time::{Duration, SystemTime}};
use std::collections::BTreeMap;

use crate::{cache::cache::Cache, errors::MyResult};

use super::cache::{Key, Locker, LockerGuard, Value};

pub struct MemCacheLockerGuard<'a, T: ?Sized + 'a>(MutexGuard<'a, T>);

impl<'a, T: ?Sized + 'a> LockerGuard<'a, T> for MemCacheLockerGuard<'a, T> {}

impl<'a, T: ?Sized + 'a> Deref for MemCacheLockerGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<'a, T: ?Sized + 'a> DerefMut for MemCacheLockerGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.deref_mut()
    }
}

type MemCacheLockResult<Guard> = Result<Guard, PoisonError<Guard>>;

pub struct MemCacheLocker<T>(Mutex<Option<T>>);

impl<T> Locker<Option<T>> for MemCacheLocker<T> {
    type LockError = PoisonError<Self::LockerGuard<'a>> where T: 'a;
    type LockerGuard<'a> = MemCacheLockerGuard<'a, Option<T>> where Self: 'a, T: 'a;

    fn lock(&self) -> Result<Self::LockerGuard<'a>, Self::LockError<'a>> {
        Ok(MemCacheLockerGuard(self.0.lock()?))
    }
    
}

pub struct MemCache {
    data: HashMap<Vec<u8>, Vec<u8>>,
    put_times: BTreeMap<SystemTime, Vec<Vec<u8>>>, // time -> keys
    keep_duration: Duration,
}

impl MemCache {
    pub fn new(keep_duration: Duration) -> Self {
        Self {
            data: HashMap::new(),
            put_times: BTreeMap::new(),
            keep_duration,
        }
    }
}

impl Cache for MemCache {
    type EntryLock = MemCacheLocker;
    fn keep_duration(&self) -> Duration {
        self.keep_duration
    }
    fn get(&mut self, key: Key) -> MyResult<Self::EntryLock> {
        // Remove expired entries.
        let time_threshold = SystemTime::now() - self.keep_duration;
        while let Some(kv) = self.put_times.first_key_value() {
            if *kv.0 < time_threshold {
                for kv2 in kv.1 {
                    self.data.remove(kv2);
                }
                self.put_times.pop_first();
            }
        }

        Ok(self.data.get(&Vec::from(key.0)))
    }
    fn put(&mut self, key: Key, value: Value) -> MyResult<()> {
        self.data.insert(Vec::from(key.0), Vec::from(value.0));
        let time = SystemTime::now();
        self.put_times
            .entry(time)
            .and_modify(|v| v.push(Vec::from(value.0)))
            .or_insert_with(|| vec![Vec::from(value.0)]);

        Ok(())
    }
}