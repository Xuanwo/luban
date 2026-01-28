use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::{Mutex, oneshot};
use tokio::time::Instant;

const IN_FLIGHT_MAX_AGE: Duration = Duration::from_secs(60);

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

        match guard.entry(key) {
            std::collections::hash_map::Entry::Occupied(mut entry) => match entry.get_mut() {
                Entry::Done { value, .. } => Begin::Done(value.clone()),
                Entry::InFlight { waiters, .. } => {
                    let (tx, rx) = oneshot::channel();
                    waiters.push(tx);
                    Begin::Wait(rx)
                }
            },
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(Entry::InFlight {
                    started_at: now,
                    waiters: Vec::new(),
                });
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

        let mut waiters = waiters;
        if let Some(last_tx) = waiters.pop() {
            for tx in waiters {
                let _ = tx.send(result.clone());
            }
            let _ = last_tx.send(result);
        }

        if guard.len() > self.max_entries {
            purge_expired(&mut guard, now);
        }
    }
}

fn purge_expired<V>(guard: &mut HashMap<String, Entry<V>>, now: Instant) {
    guard.retain(|_, entry| match entry {
        Entry::Done { expires_at, .. } => *expires_at > now,
        Entry::InFlight { started_at, .. } => now.duration_since(*started_at) < IN_FLIGHT_MAX_AGE,
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

    #[tokio::test]
    async fn idempotency_store_notifies_multiple_waiters() {
        let store = IdempotencyStore::<u64>::new(Duration::from_secs(30), 64);
        let key = "k1".to_owned();

        match store.begin(key.clone()).await {
            Begin::Owner => {}
            _ => panic!("expected owner"),
        }

        let waiter1 = match store.begin(key.clone()).await {
            Begin::Wait(rx) => rx,
            _ => panic!("expected waiter"),
        };

        let waiter2 = match store.begin(key.clone()).await {
            Begin::Wait(rx) => rx,
            _ => panic!("expected waiter"),
        };

        store.complete(key.clone(), Ok(42)).await;

        let got1 = waiter1.await.expect("waiter dropped").expect("ok");
        let got2 = waiter2.await.expect("waiter dropped").expect("ok");
        assert_eq!(got1, 42);
        assert_eq!(got2, 42);
    }
}
