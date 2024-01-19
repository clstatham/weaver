use std::{ptr::NonNull, sync::atomic::AtomicBool};

use crate::{StaticId, TypeInfo};

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

pub struct AtomicMutex {
    lock: AtomicBool,
}

impl AtomicMutex {
    pub fn new() -> Self {
        Self {
            lock: AtomicBool::new(false),
        }
    }

    pub fn try_borrow(&self) -> anyhow::Result<()> {
        match self.lock.compare_exchange_weak(
            false,
            true,
            std::sync::atomic::Ordering::Acquire,
            std::sync::atomic::Ordering::Relaxed,
        ) {
            Ok(_) => Ok(()),
            Err(_) => Err(anyhow::anyhow!("Mutex is already locked")),
        }
    }

    pub fn release(&self) {
        self.lock.store(false, std::sync::atomic::Ordering::Release);
    }
}

impl Default for AtomicMutex {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Data {
    info: TypeInfo,
    ptr: NonNull<u8>,
}

// SAFETY: `Data` is `Send` and `Sync` because `T` is always `Send` and `Sync`.
unsafe impl Send for Data {}
unsafe impl Sync for Data {}

impl Data {
    pub fn new<T: Send + Sync + 'static>(component: T) -> Self {
        let info = TypeInfo::of::<T>();
        // SAFETY: `layout` is valid because it is created from `std::alloc::Layout::new::<T>()`.
        let data = unsafe {
            let ptr = std::alloc::alloc(info.layout);
            if ptr.is_null() {
                std::alloc::handle_alloc_error(info.layout);
            }
            ptr.cast::<T>().write(component);
            NonNull::new_unchecked(ptr)
        };
        Self { ptr: data, info }
    }

    /// # Safety
    ///
    /// `self.id` must be the same as `crate::static_id::<T>()`.
    #[inline]
    pub unsafe fn as_ref_unchecked<T: Send + Sync + 'static>(&self) -> &T {
        debug_assert_eq!(self.info.id, crate::static_id::<T>());
        // SAFETY: `self.data` is a valid pointer to a `T` because `self.id` is the same as `crate::static_id::<T>()`.
        unsafe { &*(self.ptr.as_ptr() as *const T) }
    }

    /// # Safety
    ///
    /// `self.id` must be the same as `crate::static_id::<T>()`.
    #[inline]
    pub unsafe fn as_mut_unchecked<T: Send + Sync + 'static>(&mut self) -> &mut T {
        debug_assert_eq!(self.info.id, crate::static_id::<T>());
        // SAFETY: `self.data` is a valid pointer to a `T` because `self.id` is the same as `crate::static_id::<T>()`.
        unsafe { &mut *(self.ptr.as_ptr() as *mut T) }
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

impl<T: Send + Sync + 'static> AsRef<T> for Data {
    fn as_ref(&self) -> &T {
        assert_eq!(self.info.id, crate::static_id::<T>());
        // SAFETY: `self.data` is a valid pointer to a `T` because `self.id` is the same as `crate::static_id::<T>()`.
        unsafe { &*(self.ptr.as_ptr() as *const T) }
    }
}

impl<T: Send + Sync + 'static> AsMut<T> for Data {
    fn as_mut(&mut self) -> &mut T {
        assert_eq!(self.info.id, crate::static_id::<T>());
        // SAFETY: `self.data` is a valid pointer to a `T` because `self.id` is the same as `crate::static_id::<T>()`.
        unsafe { &mut *(self.ptr.as_ptr() as *mut T) }
    }
}

impl Drop for Data {
    fn drop(&mut self) {
        // SAFETY: `self.id` is the same as `crate::static_id::<T>()`.
        unsafe {
            (self.info.drop_fn)(self.ptr.as_ptr());
        }
        if self.info.layout.size() != 0 {
            // SAFETY: `self.data` is a valid pointer to a `T` because `self.id` is the same as `crate::static_id::<T>()`.
            unsafe {
                std::alloc::dealloc(self.ptr.as_ptr(), self.info.layout);
            }
        }
    }
}

// #[derive(Clone)]
// pub struct ComponentPtr {
//     pub info: TypeInfo,
//     pub component: Arc<AtomicRefCell<Data>>,
// }

// impl ComponentPtr {
//     #[inline]
//     pub fn new<T: Component>(component: T) -> Self {
//         Self {
//             info: TypeInfo::of::<T>(),
//             component: Arc::new(AtomicRefCell::new(Data::new(component))),
//         }
//     }

//     #[inline]
//     pub fn from_data(data: Data) -> Self {
//         Self {
//             info: data.info,
//             component: Arc::new(AtomicRefCell::new(data)),
//         }
//     }

//     #[inline]
//     pub fn id(&self) -> StaticId {
//         self.info.id
//     }

//     #[inline]
//     pub fn name(&self) -> &'static str {
//         self.info.name
//     }

//     #[inline]
//     pub fn info(&self) -> TypeInfo {
//         self.info
//     }

//     #[inline]
//     pub fn borrow_as_ref<T: Component>(&self) -> AtomicRef<'_, T> {
//         assert_eq!(self.info.id, crate::static_id::<T>());
//         // SAFETY: `self.component` is a valid pointer to a `T` because `self.id` is the same as `crate::static_id::<T>()`.
//         unsafe { self.borrow_as_ref_unchecked() }
//     }

//     #[inline]
//     pub fn borrow_as_mut<T: Component>(&self) -> AtomicRefMut<'_, T> {
//         assert_eq!(self.info.id, crate::static_id::<T>());
//         // SAFETY: `self.component` is a valid pointer to a `T` because `self.id` is the same as `crate::static_id::<T>()`.
//         unsafe { self.borrow_as_mut_unchecked() }
//     }

//     /// # Safety
//     ///
//     /// `self.id` must be the same as `crate::static_id::<T>()`.
//     #[inline(never)]
//     pub unsafe fn borrow_as_ref_unchecked<T: Component>(&self) -> AtomicRef<'_, T> {
//         debug_assert_eq!(self.info.id, crate::static_id::<T>());
//         // SAFETY: `self.component` is a valid pointer to a `T` because `self.id` is the same as `crate::static_id::<T>()`.
//         let lock = self.component.borrow();
//         AtomicRef::map(lock, |component| unsafe {
//             component.as_ref_unchecked::<T>()
//         })
//     }

//     /// # Safety
//     ///
//     /// `self.id` must be the same as `crate::static_id::<T>()`.
//     #[inline(never)]
//     pub unsafe fn borrow_as_mut_unchecked<T: Component>(&self) -> AtomicRefMut<'_, T> {
//         debug_assert_eq!(self.info.id, crate::static_id::<T>());
//         // SAFETY: `self.component` is a valid pointer to a `T` because `self.id` is the same as `crate::static_id::<T>()`.
//         let lock = self.component.borrow_mut();
//         AtomicRefMut::map(lock, |component| unsafe {
//             component.as_mut_unchecked::<T>()
//         })
//     }
// }

// impl Debug for ComponentPtr {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.debug_struct("ComponentPtr")
//             .field("component_id", &self.info.id)
//             .field("component_name", &self.info.name)
//             .finish()
//     }
// }
