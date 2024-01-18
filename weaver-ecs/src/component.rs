use std::{
    alloc::Layout,
    fmt::Debug,
    ptr::NonNull,
    sync::{atomic::AtomicBool, Arc},
};

use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};

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

pub struct DynComponent {
    info: TypeInfo,
    ptr: NonNull<u8>,
}

// SAFETY: `DynComponent` is `Send` and `Sync` because `T` is always a `Component`, which requires `Send` and `Sync`.
unsafe impl Send for DynComponent {}
unsafe impl Sync for DynComponent {}

impl DynComponent {
    pub fn new<T: Component>(component: T) -> Self {
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
    pub unsafe fn as_ref_unchecked<T: Component>(&self) -> &T {
        debug_assert_eq!(self.info.id, crate::static_id::<T>());
        // SAFETY: `self.data` is a valid pointer to a `T` because `self.id` is the same as `crate::static_id::<T>()`.
        unsafe { &*(self.ptr.as_ptr() as *const T) }
    }

    /// # Safety
    ///
    /// `self.id` must be the same as `crate::static_id::<T>()`.
    #[inline]
    pub unsafe fn as_mut_unchecked<T: Component>(&mut self) -> &mut T {
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
}

impl<T: Component> AsRef<T> for DynComponent {
    fn as_ref(&self) -> &T {
        assert_eq!(self.info.id, crate::static_id::<T>());
        // SAFETY: `self.data` is a valid pointer to a `T` because `self.id` is the same as `crate::static_id::<T>()`.
        unsafe { &*(self.ptr.as_ptr() as *const T) }
    }
}

impl<T: Component> AsMut<T> for DynComponent {
    fn as_mut(&mut self) -> &mut T {
        assert_eq!(self.info.id, crate::static_id::<T>());
        // SAFETY: `self.data` is a valid pointer to a `T` because `self.id` is the same as `crate::static_id::<T>()`.
        unsafe { &mut *(self.ptr.as_ptr() as *mut T) }
    }
}

impl Drop for DynComponent {
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

#[derive(Clone)]
pub struct ComponentPtr {
    pub type_info: Arc<TypeInfo>,
    pub component: Arc<AtomicRefCell<DynComponent>>,
}

impl ComponentPtr {
    #[inline]
    pub fn new<T: Component>(component: T) -> Self {
        Self {
            type_info: Arc::new(TypeInfo::of::<T>()),
            component: Arc::new(AtomicRefCell::new(DynComponent::new(component))),
        }
    }

    #[inline]
    pub fn id(&self) -> StaticId {
        self.type_info.id
    }

    #[inline]
    pub fn name(&self) -> &'static str {
        self.type_info.name
    }

    #[inline]
    pub fn borrow_as_ref<T: Component>(&self) -> AtomicRef<'_, T> {
        assert_eq!(self.type_info.id, crate::static_id::<T>());
        // SAFETY: `self.component` is a valid pointer to a `T` because `self.id` is the same as `crate::static_id::<T>()`.
        AtomicRef::map(self.component.borrow(), |component| unsafe {
            component.as_ref_unchecked::<T>()
        })
    }

    #[inline]
    pub fn borrow_as_mut<T: Component>(&self) -> AtomicRefMut<'_, T> {
        assert_eq!(self.type_info.id, crate::static_id::<T>());
        // SAFETY: `self.component` is a valid pointer to a `T` because `self.id` is the same as `crate::static_id::<T>()`.
        AtomicRefMut::map(self.component.borrow_mut(), |component| unsafe {
            component.as_mut_unchecked::<T>()
        })
    }

    /// # Safety
    ///
    /// `self.id` must be the same as `crate::static_id::<T>()`.
    #[inline]
    pub unsafe fn borrow_as_ref_unchecked<T: Component>(&self) -> AtomicRef<'_, T> {
        debug_assert_eq!(self.type_info.id, crate::static_id::<T>());
        // SAFETY: `self.component` is a valid pointer to a `T` because `self.id` is the same as `crate::static_id::<T>()`.
        AtomicRef::map(self.component.borrow(), |component| unsafe {
            component.as_ref_unchecked::<T>()
        })
    }

    /// # Safety
    ///
    /// `self.id` must be the same as `crate::static_id::<T>()`.
    #[inline]
    pub unsafe fn borrow_as_mut_unchecked<T: Component>(&self) -> AtomicRefMut<'_, T> {
        debug_assert_eq!(self.type_info.id, crate::static_id::<T>());
        // SAFETY: `self.component` is a valid pointer to a `T` because `self.id` is the same as `crate::static_id::<T>()`.
        AtomicRefMut::map(self.component.borrow_mut(), |component| unsafe {
            component.as_mut_unchecked::<T>()
        })
    }
}

impl Debug for ComponentPtr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentPtr")
            .field("component_id", &self.type_info.id)
            .field("component_name", &self.type_info.name)
            .finish()
    }
}

struct RawWhateverVec {
    pub(crate) ptr: NonNull<u8>,
    pub(crate) cap: usize,
    pub(crate) info: TypeInfo,
}

// SAFETY: `RawWhateverVec` is `Send` and `Sync` because `T` is always `Send` and `Sync`.
unsafe impl Send for RawWhateverVec {}
unsafe impl Sync for RawWhateverVec {}

impl RawWhateverVec {
    pub fn new<T: Send + Sync + 'static>() -> Self {
        let info = TypeInfo::of::<T>();
        let cap = if info.layout.size() == 0 {
            usize::MAX
        } else {
            1
        };
        Self {
            ptr: NonNull::dangling(),
            cap,
            info,
        }
    }

    fn grow(&mut self) {
        assert!(
            self.info.layout.size() != 0,
            "Cannot grow a zero-sized RawDynVec"
        );
        let (new_cap, new_layout) = if self.cap == 0 {
            (1, self.info.layout)
        } else {
            let new_cap = self.cap * 2;
            let new_layout = Layout::from_size_align(
                self.info.layout.size() * new_cap,
                self.info.layout.align(),
            )
            .unwrap();
            (new_cap, new_layout)
        };

        assert!(
            new_layout.size() <= isize::MAX as usize,
            "Cannot allocate more than isize::MAX bytes"
        );

        // SAFETY: `new_layout` is valid because it is created from `self.info.layout`.
        let new_ptr = if self.cap == 0 {
            unsafe { std::alloc::alloc(new_layout) }
        } else {
            let old_layout = Layout::from_size_align(
                self.info.layout.size() * self.cap,
                self.info.layout.align(),
            )
            .unwrap();
            // SAFETY: `self.ptr` is a valid pointer to a `T` because `self.id` is the same as `crate::static_id::<T>()`.
            unsafe { std::alloc::realloc(self.ptr.as_ptr(), old_layout, new_layout.size()) }
        };

        self.ptr = match NonNull::new(new_ptr.cast()) {
            Some(ptr) => ptr,
            None => std::alloc::handle_alloc_error(new_layout),
        };
        self.cap = new_cap;
    }

    #[inline]
    pub const fn id(&self) -> StaticId {
        self.info.id
    }

    #[inline]
    pub const fn name(&self) -> &'static str {
        self.info.name
    }
}

impl Drop for RawWhateverVec {
    fn drop(&mut self) {
        if self.info.layout.size() != 0 && self.cap != 0 {
            // SAFETY: `self.ptr` is a valid pointer to a `T` because `self.id` is the same as `crate::static_id::<T>()`.
            unsafe {
                std::alloc::dealloc(
                    self.ptr.as_ptr(),
                    Layout::from_size_align(
                        self.info.layout.size() * self.cap,
                        self.info.layout.align(),
                    )
                    .unwrap(),
                )
            };
        }
    }
}

/// A type-erased vector of a single type of data.
///
/// This differs from a `Vec<T: Send + Sync + 'static>` in that it stores the TypeInfo rather than using a generic parameter.
///
/// This differs from a `Vec<dyn Send + Sync + 'static>` in that it stores only one type of data, rather than any type of data.
pub struct WhateverVec {
    buf: RawWhateverVec,
    len: usize,
}

impl WhateverVec {
    pub fn new<T: Send + Sync + 'static>() -> Self {
        Self {
            buf: RawWhateverVec::new::<T>(),
            len: 0,
        }
    }

    pub fn push<T: Send + Sync + 'static>(&mut self, component: T) {
        assert_eq!(self.buf.info.id, crate::static_id::<T>());

        unsafe {
            self.push_unchecked(component);
        }
    }

    pub fn pop<T: Send + Sync + 'static>(&mut self) -> Option<T> {
        assert_eq!(self.buf.info.id, crate::static_id::<T>());

        unsafe { self.pop_unchecked() }
    }

    pub fn get<T: Send + Sync + 'static>(&self, index: usize) -> Option<&T> {
        assert_eq!(self.buf.info.id, crate::static_id::<T>());

        unsafe { self.get_unchecked(index) }
    }

    pub fn get_mut<T: Send + Sync + 'static>(&mut self, index: usize) -> Option<&mut T> {
        assert_eq!(self.buf.info.id, crate::static_id::<T>());

        unsafe { self.get_mut_unchecked(index) }
    }

    /// # Safety
    ///
    /// `self.buf.id` must be the same as `crate::static_id::<T>()`.
    pub unsafe fn push_unchecked<T: Send + Sync + 'static>(&mut self, component: T) {
        debug_assert_eq!(self.buf.info.id, crate::static_id::<T>());

        if self.len == self.buf.cap {
            self.buf.grow();
        }

        unsafe {
            self.buf
                .ptr
                .as_ptr()
                .add(self.len * self.buf.info.layout.size())
                .cast::<T>()
                .write(component);
        }
        self.len += 1;
    }

    /// # Safety
    ///
    /// `self.buf.id` must be the same as `crate::static_id::<T>()`.
    pub unsafe fn pop_unchecked<T: Send + Sync + 'static>(&mut self) -> Option<T> {
        debug_assert_eq!(self.buf.info.id, crate::static_id::<T>());

        if self.len == 0 {
            return None;
        }
        self.len -= 1;

        unsafe {
            Some(
                self.buf
                    .ptr
                    .as_ptr()
                    .add(self.len * self.buf.info.layout.size())
                    .cast::<T>()
                    .read(),
            )
        }
    }

    /// # Safety
    ///
    /// `self.buf.id` must be the same as `crate::static_id::<T>()`.
    pub unsafe fn get_unchecked<T: Send + Sync + 'static>(&self, index: usize) -> Option<&T> {
        debug_assert_eq!(self.buf.info.id, crate::static_id::<T>());

        if index >= self.len {
            return None;
        }

        unsafe {
            Some(
                &*self
                    .buf
                    .ptr
                    .as_ptr()
                    .add(index * self.buf.info.layout.size())
                    .cast::<T>(),
            )
        }
    }

    /// # Safety
    ///
    /// `self.buf.id` must be the same as `crate::static_id::<T>()`.
    pub unsafe fn get_mut_unchecked<T: Send + Sync + 'static>(
        &mut self,
        index: usize,
    ) -> Option<&mut T> {
        debug_assert_eq!(self.buf.info.id, crate::static_id::<T>());

        if index >= self.len {
            return None;
        }

        unsafe {
            Some(
                &mut *self
                    .buf
                    .ptr
                    .as_ptr()
                    .add(index * self.buf.info.layout.size())
                    .cast::<T>(),
            )
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len != 0
    }

    pub fn iter<T: Send + Sync + 'static>(&self) -> impl Iterator<Item = &T> {
        assert_eq!(self.buf.info.id, crate::static_id::<T>());
        // SAFETY: `self.buf.ptr` is a valid pointer to a `T` because `self.buf.id` is the same as `crate::static_id::<T>()`.
        (0..self.len).map(move |index| unsafe { self.get_unchecked(index).unwrap() })
    }
}

impl Drop for WhateverVec {
    fn drop(&mut self) {
        // move the elements out of the vector
        if self.buf.info.layout.size() != 0 {
            for i in 0..self.len {
                // SAFETY: `ptr` is a valid pointer to a `T` because `self.buf.id` is the same as `crate::static_id::<T>()`.
                unsafe {
                    let ptr = self.buf.ptr.as_ptr().add(i * self.buf.info.layout.size());
                    (self.buf.info.drop_fn)(ptr);
                }
            }
        }

        // RawWhateverVec::drop will handle deallocating the memory
    }
}
