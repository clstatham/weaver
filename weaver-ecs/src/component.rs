use std::ptr::NonNull;

use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};

use crate::{static_id, StaticId, TypeInfo};

pub trait Downcast: std::any::Any {
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

impl<T: std::any::Any> Downcast for T {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// A component is a data structure that can be attached to an entity.
pub trait Component: Send + Sync + 'static {}

/// A unique pointer to a type-erased component.
pub struct Data {
    info: TypeInfo,
    data: NonNull<u8>,
}

unsafe impl Send for Data {}
unsafe impl Sync for Data {}

impl Data {
    pub fn new<T: Send + Sync + 'static>(data: T) -> Self {
        let info = TypeInfo::of::<T>();
        if info.id == static_id::<Data>() {
            panic!("Cannot create a Data from a Data")
        }
        let data = Box::new(data);
        let data = unsafe { NonNull::new_unchecked(Box::into_raw(data) as *mut u8) };
        // let data = Box::into_raw(data) as *mut u8;
        Self { data, info }
    }

    /// # Safety
    ///
    /// `self.id` must be the same as `crate::static_id::<T>()`.
    ///
    /// There must be no mutable references to the data.
    #[inline]
    pub unsafe fn as_ref_unchecked<T: Send + Sync + 'static>(&self) -> &T {
        debug_assert_eq!(self.info.id, crate::static_id::<T>());
        // SAFETY: `self.data` is a valid pointer to a `T` because `self.id` is the same as `crate::static_id::<T>()`.
        unsafe { self.data.cast::<T>().as_ref() }
    }

    /// # Safety
    ///
    /// `self.id` must be the same as `crate::static_id::<T>()`.
    ///
    /// There must be no other references to the data.
    #[inline]
    pub unsafe fn as_mut_unchecked<T: Send + Sync + 'static>(&mut self) -> &mut T {
        debug_assert_eq!(self.info.id, crate::static_id::<T>());
        // SAFETY: `self.data` is a valid pointer to a `T` because `self.id` is the same as `crate::static_id::<T>()`.
        unsafe { self.data.cast::<T>().as_mut() }
    }

    #[inline]
    pub const fn id(&self) -> StaticId {
        self.info.id
    }

    #[inline]
    pub const fn name(&self) -> &'static str {
        self.info.name
    }

    #[inline]
    pub fn info(&self) -> TypeInfo {
        self.info
    }
}

impl Drop for Data {
    fn drop(&mut self) {
        // SAFETY: `self.id` is the same as `crate::static_id::<T>()`.
        unsafe {
            (self.info.drop_fn)(self.data.as_ptr());
        }
        if self.info.layout.size() != 0 {
            // SAFETY: `self.data` is a valid pointer to a `T` because `self.id` is the same as `crate::static_id::<T>()`.
            unsafe {
                std::alloc::dealloc(self.data.as_ptr(), self.info.layout);
            }
        }
    }
}

pub struct LockedData {
    data: AtomicRefCell<Data>,
}

impl LockedData {
    pub fn new(data: Data) -> Self {
        Self {
            data: AtomicRefCell::new(data),
        }
    }

    #[inline]
    pub fn borrow(&self) -> AtomicRef<'_, Data> {
        self.data.borrow()
    }

    #[inline]
    pub fn borrow_mut(&self) -> AtomicRefMut<'_, Data> {
        self.data.borrow_mut()
    }

    pub fn into_inner(self) -> Data {
        self.data.into_inner()
    }
}
