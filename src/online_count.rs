use std::sync::{Arc, RwLock};

pub struct OnlineCount {
    count: Option<Arc<RwLock<usize>>>,
}

impl Default for OnlineCount {
    fn default() -> Self {
        Self {
            count: Default::default(),
        }
    }
}

impl OnlineCount {
    pub fn new() -> Self {
        Self {
            count: Some(Arc::new(RwLock::new(0))),
        }
    }

    pub fn add(&self) -> Self {
        if let Some(count) = &self.count {
            if let Ok(mut v) = count.write() {
                *v = v.checked_add(1).unwrap_or(0);
            }
        }
        Self {
            count: self.count.clone(),
        }
    }

    pub fn now(&self) -> usize {
        if let Some(count) = &self.count {
            if let Ok(v) = count.read() {
                return *v;
            }
        }
        0
    }
}

impl Drop for OnlineCount {
    fn drop(&mut self) {
        if let Some(count) = &self.count {
            if let Ok(mut v) = count.write() {
                *v = v.checked_sub(1).unwrap_or(0);
            }
        }
    }
}
