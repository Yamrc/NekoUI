use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ENTITY_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Entity<T> {
    id: u64,
    marker: PhantomData<fn() -> T>,
}

impl<T> Copy for Entity<T> {}

impl<T> Clone for Entity<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Entity<T> {
    pub(super) fn new() -> Self {
        Self {
            id: NEXT_ENTITY_ID.fetch_add(1, Ordering::Relaxed),
            marker: PhantomData,
        }
    }

    pub(in crate::app) const fn from_raw(id: u64) -> Self {
        Self {
            id,
            marker: PhantomData,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn downgrade(self) -> WeakEntity<T> {
        WeakEntity {
            id: self.id,
            marker: PhantomData,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct WeakEntity<T> {
    id: u64,
    marker: PhantomData<fn() -> T>,
}

impl<T> Copy for WeakEntity<T> {}

impl<T> Clone for WeakEntity<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> WeakEntity<T> {
    pub fn id(&self) -> u64 {
        self.id
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct View<T> {
    id: u64,
    marker: PhantomData<fn() -> T>,
}

impl<T> Copy for View<T> {}

impl<T> Clone for View<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> View<T> {
    pub(super) fn new() -> Self {
        Self {
            id: NEXT_ENTITY_ID.fetch_add(1, Ordering::Relaxed),
            marker: PhantomData,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn entity(self) -> Entity<T> {
        Entity::from_raw(self.id)
    }
}
