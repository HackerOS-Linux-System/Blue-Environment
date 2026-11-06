use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub trait LockExt<T> {
    fn lock_safe(&self) -> MutexGuard<'_, T>;
}

impl<T> LockExt<T> for Mutex<T> {
    fn lock_safe(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("recovered a poisoned mutex");
            poisoned.into_inner()
        })
    }
}

#[allow(dead_code)] // provided for future RwLock users; covered by the same recovery logic
pub trait RwLockExt<T> {
    fn read_safe(&self) -> RwLockReadGuard<'_, T>;
    fn write_safe(&self) -> RwLockWriteGuard<'_, T>;
}

#[allow(dead_code)]
impl<T> RwLockExt<T> for RwLock<T> {
    fn read_safe(&self) -> RwLockReadGuard<'_, T> {
        self.read().unwrap_or_else(|p| p.into_inner())
    }
    fn write_safe(&self) -> RwLockWriteGuard<'_, T> {
        self.write().unwrap_or_else(|p| p.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn poisoned_mutex_is_recovered_not_propagated() {
        let m = Arc::new(Mutex::new(vec![1, 2, 3]));
        let m2 = m.clone();
        let _ = std::thread::spawn(move || {
            let mut g = m2.lock().expect("first lock");
            g.push(4);
            panic!("poison it");
        })
        .join();
        assert!(m.is_poisoned());
        let g = m.lock_safe();
        assert_eq!(*g, vec![1, 2, 3, 4], "data written before the panic must survive");
    }

    #[test]
    fn healthy_mutex_behaves_normally() {
        let m = Mutex::new(5);
        *m.lock_safe() += 1;
        assert_eq!(*m.lock_safe(), 6);
    }
}
