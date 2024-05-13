use std::{collections::HashMap, time::{Duration, SystemTime}};
use std::collections::BTreeMap;

use crate::{cache::cache::Cache, errors::MyResult};

use super::cache::{Key, Value};

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
    fn keep_duration(&self) -> Duration {
        self.keep_duration
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

    fn get(&mut self, key: Key) -> MyResult<Option<&Vec<u8>>> {
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

        Ok(self.data.get(&Vec::from(key.0))) // TODO: inefficient?
    }
}