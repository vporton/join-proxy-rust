use std::{collections::HashMap, ops::{Deref, DerefMut}, sync::PoisonError, time::{Duration, SystemTime}};
use std::collections::BTreeMap;
use std::cmp::{Eq, Hash};

use actix_web::guard::Guard;
use lock_api::{GuardNoSend,};
use lockable::{Lockable, LockableHashMap};

use crate::{cache::cache::Cache, errors::MyResult};

pub struct MemCache {
    data: LockableHashMap<Vec<u8>, Vec<u8>>,
    put_times: BTreeMap<SystemTime, Vec<Vec<u8>>>, // time -> keys
    keep_duration: Duration,
}

impl MemCache {
    pub fn new(keep_duration: Duration) -> Self {
        Self {
            data: LockableHashMap::new(),
            put_times: BTreeMap::new(),
            keep_duration,
        }
    }
}

impl<K, V> Cache<K, V> for MemCache {
    type Guard<'a> = <LockableHashMap<K, V> as Lockable<K, V>>::Guard<'a>
        where
            K: 'a + Eq + Hash,
            V: 'a;
    fn keep_duration(&self) -> Duration {
        self.keep_duration
    }
    fn get(&mut self, key: K) -> MyResult<parking_lot::Mutex<Vec<u8>>> {
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

        let data = self.data.get(&Vec::from(key.0));
        Ok(if let Some(data) = data {
            if let Some(data) = data {
                Some(data)
            } else {
                
            }
        })
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