use std::{
    alloc::{self, Layout},
    any::TypeId,
    ptr::NonNull,
};

use crate::component::Component;

/// A type-erased, growable array of one component type, laid out exactly like a `Vec<T>`.
pub struct ComponentVec {
    type_id: TypeId,
    type_name: &'static str,
    item_layout: Layout,
    /// Drops one element in place, or `None` if the type doesn't need dropping.
    drop_fn: Option<unsafe fn(*mut u8)>,
    data: NonNull<u8>,
    len: usize,
    capacity: usize,
}

// SAFETY: a `ComponentVec` can only be created for a `T: Component`, which is `Send + Sync`
unsafe impl Send for ComponentVec {}
// SAFETY: see above
unsafe impl Sync for ComponentVec {}

unsafe fn drop_in_place<T>(ptr: *mut u8) {
    // SAFETY: the caller passes a pointer to a live, aligned `T`
    unsafe { ptr.cast::<T>().drop_in_place() }
}

/// A non-null pointer with the right alignment for `layout`, for when nothing is allocated.
fn dangling(layout: Layout) -> NonNull<u8> {
    NonNull::new(std::ptr::without_provenance_mut(layout.align())).unwrap()
}

impl ComponentVec {
    pub fn new<T: Component>() -> Self {
        let item_layout = Layout::new::<T>();
        Self {
            type_id: TypeId::of::<T>(),
            type_name: std::any::type_name::<T>(),
            item_layout,
            drop_fn: std::mem::needs_drop::<T>().then_some(drop_in_place::<T> as unsafe fn(*mut u8)),
            data: dangling(item_layout),
            len: 0,
            capacity: if item_layout.size() == 0 { usize::MAX } else { 0 },
        }
    }

    /// Creates an empty vec for the same component type.
    pub fn clone_empty(&self) -> Self {
        Self {
            data: dangling(self.item_layout),
            len: 0,
            capacity: if self.item_layout.size() == 0 { usize::MAX } else { 0 },
            ..*self
        }
    }

    pub fn element_typeid(&self) -> TypeId {
        self.type_id
    }

    pub fn element_type_name(&self) -> &'static str {
        self.type_name
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn assert_type<T: Component>(&self) {
        assert_eq!(
            self.type_id,
            TypeId::of::<T>(),
            "ComponentVec holds {}, not {}",
            self.type_name,
            std::any::type_name::<T>()
        );
    }

    fn array_layout(&self, capacity: usize) -> Layout {
        let size = self
            .item_layout
            .size()
            .checked_mul(capacity)
            .expect("ComponentVec capacity overflow");
        Layout::from_size_align(size, self.item_layout.align()).expect("ComponentVec capacity overflow")
    }

    /// Makes room for at least one more element. Zero-sized types never allocate.
    fn reserve_one(&mut self) {
        if self.len < self.capacity {
            return;
        }
        let new_capacity = (self.capacity * 2).max(4);
        let new_layout = self.array_layout(new_capacity);
        let ptr = if self.capacity == 0 {
            // SAFETY: the size is nonzero, since zero-sized types never get here
            unsafe { alloc::alloc(new_layout) }
        } else {
            let old_layout = self.array_layout(self.capacity);
            // SAFETY: `data` was allocated with `old_layout`, and the new size is nonzero
            unsafe { alloc::realloc(self.data.as_ptr(), old_layout, new_layout.size()) }
        };
        self.data = NonNull::new(ptr).unwrap_or_else(|| alloc::handle_alloc_error(new_layout));
        self.capacity = new_capacity;
    }

    /// Pointer to the element at `index`, which may be one past the end.
    ///
    /// # Safety
    ///
    /// `index <= capacity`.
    unsafe fn ptr_at(&self, index: usize) -> *mut u8 {
        // SAFETY: in bounds of the allocation (or a zero offset for zero-sized types)
        unsafe { self.data.as_ptr().add(index * self.item_layout.size()) }
    }

    pub fn push<T: Component>(&mut self, value: T) {
        self.assert_type::<T>();
        self.reserve_one();
        // SAFETY: `reserve_one` made room at `len`, and the type matches
        unsafe { self.ptr_at(self.len).cast::<T>().write(value) };
        self.len += 1;
    }

    pub fn pop<T: Component>(&mut self) -> Option<T> {
        self.assert_type::<T>();
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        // SAFETY: the element at the old last index is live, the type matches, and decrementing
        // `len` first hands ownership to the caller
        Some(unsafe { self.ptr_at(self.len).cast::<T>().read() })
    }

    /// Moves the element at `index` to the end of `dst`, filling the hole with this vec's last element.
    pub fn swap_remove_into(&mut self, index: usize, dst: &mut ComponentVec) {
        assert_eq!(
            self.type_id, dst.type_id,
            "can't move a {} into a ComponentVec of {}",
            self.type_name, dst.type_name
        );
        assert!(index < self.len, "index {index} out of bounds (len {})", self.len);
        dst.reserve_one();
        let size = self.item_layout.size();
        let last = self.len - 1;
        // SAFETY: `index` and `last` are live elements, `dst` has room at its `len`, the two vecs
        // are different allocations, and the moved-out slots are forgotten by shrinking `len`
        unsafe {
            let hole = self.ptr_at(index);
            std::ptr::copy_nonoverlapping(hole, dst.ptr_at(dst.len), size);
            if index != last {
                std::ptr::copy_nonoverlapping(self.ptr_at(last), hole, size);
            }
        }
        dst.len += 1;
        self.len = last;
    }

    /// Moves this vec's last element to the end of `dst`. Returns false if this vec is empty.
    pub fn pop_into(&mut self, dst: &mut ComponentVec) -> bool {
        if self.len == 0 {
            return false;
        }
        self.swap_remove_into(self.len - 1, dst);
        true
    }

    /// Typed view of the elements, or `None` if this vec doesn't hold `T`s.
    pub fn as_slice<T: Component>(&self) -> Option<&[T]> {
        // SAFETY: the type was just checked
        (self.type_id == TypeId::of::<T>()).then(|| unsafe { self.as_slice_unchecked() })
    }

    /// # Safety
    ///
    /// This vec must hold `T`s.
    pub unsafe fn as_slice_unchecked<T: Component>(&self) -> &[T] {
        // SAFETY: `data` is aligned and non-null, and the first `len` elements are live `T`s
        unsafe { std::slice::from_raw_parts(self.data.as_ptr().cast::<T>(), self.len) }
    }

    /// Raw pointer to the first element. Writing through it needs exclusive access to the
    /// elements, but doesn't need `&mut self`, since the buffer is a separate allocation.
    pub fn as_ptr(&self) -> *mut u8 {
        self.data.as_ptr()
    }
}

impl Drop for ComponentVec {
    fn drop(&mut self) {
        let len = std::mem::take(&mut self.len);
        if let Some(drop_fn) = self.drop_fn {
            for index in 0..len {
                // SAFETY: each of the first `len` elements is live and dropped once; `len` is
                // already zero, so a panicking drop leaks the rest instead of double-dropping
                unsafe { drop_fn(self.ptr_at(index)) };
            }
        }
        if self.item_layout.size() != 0 && self.capacity != 0 {
            // SAFETY: `data` was allocated with this layout
            unsafe { alloc::dealloc(self.data.as_ptr(), self.array_layout(self.capacity)) };
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::ComponentVec;

    /// Counts its drops, to catch leaks and double drops.
    struct Tracked(Arc<AtomicUsize>);

    impl Drop for Tracked {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn grows_and_reads_back() {
        let mut vec = ComponentVec::new::<u64>();
        for i in 0..1000 {
            vec.push(i as u64);
        }
        assert_eq!(vec.len(), 1000);
        assert_eq!(vec.as_slice::<u64>().unwrap(), (0..1000).collect::<Vec<u64>>());
        assert!(vec.as_slice::<u32>().is_none());
        assert_eq!(vec.pop::<u64>(), Some(999));
    }

    #[test]
    fn every_element_is_dropped_exactly_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut vec = ComponentVec::new::<Tracked>();
        let mut other = vec.clone_empty();
        for _ in 0..10 {
            vec.push(Tracked(drops.clone()));
        }

        vec.swap_remove_into(2, &mut other); // moved, not dropped
        drop(vec.pop::<Tracked>()); // 1 drop
        assert!(vec.pop_into(&mut other));
        assert_eq!(drops.load(Ordering::SeqCst), 1);

        drop(vec); // 7 left
        assert_eq!(drops.load(Ordering::SeqCst), 8);
        drop(other); // 2 moved
        assert_eq!(drops.load(Ordering::SeqCst), 10);
    }

    #[test]
    fn swap_remove_into_fills_the_hole_with_the_last_element() {
        let mut vec = ComponentVec::new::<String>();
        let mut other = vec.clone_empty();
        for s in ["a", "b", "c", "d"] {
            vec.push(s.to_string());
        }
        vec.swap_remove_into(1, &mut other);
        assert_eq!(vec.as_slice::<String>().unwrap(), ["a", "d", "c"]);
        assert_eq!(other.as_slice::<String>().unwrap(), ["b"]);

        vec.swap_remove_into(2, &mut other); // removing the last element moves nothing else
        assert_eq!(vec.as_slice::<String>().unwrap(), ["a", "d"]);
        assert_eq!(other.as_slice::<String>().unwrap(), ["b", "c"]);
    }

    #[test]
    fn zero_sized_types_never_allocate() {
        struct Marker;
        let mut vec = ComponentVec::new::<Marker>();
        let mut other = vec.clone_empty();
        for _ in 0..100 {
            vec.push(Marker);
        }
        vec.swap_remove_into(50, &mut other);
        assert_eq!(vec.len(), 99);
        assert_eq!(other.len(), 1);
        assert_eq!(vec.as_slice::<Marker>().unwrap().len(), 99);
        assert!(vec.pop::<Marker>().is_some());
    }

    #[test]
    fn over_aligned_types_stay_aligned() {
        #[repr(align(64))]
        struct Aligned(u8);
        let mut vec = ComponentVec::new::<Aligned>();
        for i in 0..20 {
            vec.push(Aligned(i));
        }
        let slice = vec.as_slice::<Aligned>().unwrap();
        assert_eq!(slice.as_ptr() as usize % 64, 0);
        assert_eq!(slice[19].0, 19);
    }

    #[test]
    #[should_panic(expected = "holds u32, not u64")]
    fn pushing_the_wrong_type_panics() {
        let mut vec = ComponentVec::new::<u32>();
        vec.push(1u64);
    }

    #[test]
    #[should_panic(expected = "can't move")]
    fn moving_between_different_types_panics() {
        let mut a = ComponentVec::new::<u32>();
        let mut b = ComponentVec::new::<u64>();
        a.push(1u32);
        a.swap_remove_into(0, &mut b);
    }
}
