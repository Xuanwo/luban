use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::{Mutex, oneshot};
use tokio::time::Instant;

#[derive(Clone)]
pub(crate) struct IdempotencyStore<V> {
    inner: std::sync::Arc<Mutex<HashMap<String, Entry<V>>>>,
    ttl: Duration,
    max_entries: usize,
}

enum Entry<V> {
    InFlight {
        started_at: Instant,
        waiters: Vec<oneshot::Sender<Result<V, String>>>,
    },
    Done {
        expires_at: Instant,
        value: V,
    },
}

pub(crate) enum Begin<V> {
    Owner,
    Done(V),
    Wait(oneshot::Receiver<Result<V, String>>),
}

impl<V: Clone> IdempotencyStore<V> {
    pub(crate) fn new(ttl: Duration, max_entries: usize) -> Self {
        Self {
            inner: std::sync::Arc::new(Mutex::new(HashMap::new())),
            ttl,
            max_entries,
        }
    }

    pub(crate) async fn begin(&self, key: String) -> Begin<V> {
        let now = Instant::now();
        let mut guard = self.inner.lock().await;
        purge_expired(&mut guard, now);

        match guard.get_mut(&key) {
            Some(Entry::Done { value, .. }) => Begin::Done(value.clone()),
            Some(Entry::InFlight { waiters, .. }) => {
                let (tx, rx) = oneshot::channel();
                waiters.push(tx);
                Begin::Wait(rx)
            }
            None => {
                guard.insert(
                    key,
                    Entry::InFlight {
                        started_at: now,
                        waiters: Vec::new(),
                    },
                );
                if guard.len() > self.max_entries {
                    purge_expired(&mut guard, now);
                }
                Begin::Owner
            }
        }
    }

    pub(crate) async fn complete(&self, key: String, result: Result<V, String>) {
        let now = Instant::now();
        let mut guard = self.inner.lock().await;

        let waiters = match guard.remove(&key) {
            Some(Entry::InFlight { waiters, .. }) => waiters,
            Some(Entry::Done { .. }) | None => Vec::new(),
        };

        if let Ok(value) = &result {
            let expires_at = now + self.ttl;
            guard.insert(
                key,
                Entry::Done {
                    expires_at,
                    value: value.clone(),
                },
            );
        }

        for tx in waiters {
            let _ = tx.send(result.clone());
        }

        if guard.len() > self.max_entries {
            purge_expired(&mut guard, now);
        }
    }
}

fn purge_expired<V>(guard: &mut HashMap<String, Entry<V>>, now: Instant) {
    guard.retain(|_, entry| match entry {
        Entry::Done { expires_at, .. } => *expires_at > now,
        Entry::InFlight { started_at, .. } => {
            now.duration_since(*started_at) < Duration::from_secs(60)
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn idempotency_store_deduplicates_in_flight() {
        let store = IdempotencyStore::<u64>::new(Duration::from_secs(30), 64);
        let key = "k1".to_owned();

        match store.begin(key.clone()).await {
            Begin::Owner => {}
            _ => panic!("expected owner"),
        }

        let waiter = match store.begin(key.clone()).await {
            Begin::Wait(rx) => rx,
            _ => panic!("expected waiter"),
        };

        store.complete(key.clone(), Ok(42)).await;

        let got = waiter.await.expect("waiter dropped").expect("ok");
        assert_eq!(got, 42);

        match store.begin(key.clone()).await {
            Begin::Done(v) => assert_eq!(v, 42),
            _ => panic!("expected done"),
        }
    }
}
