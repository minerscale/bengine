use std::sync::Arc;

/// # Safety
/// For all `T: FromRaw` and all `ptr: T`, `T::from_raw(T::into_raw(ptr))` must be valid and return a value identical to `ptr`
pub unsafe trait FromRaw: Sized {
    fn into_raw(self) -> *mut ();
    /// # Safety
    /// `p` must have been obtained by calling `into_raw` and must not have already been used to call `from_raw`
    unsafe fn from_raw(p: *mut ()) -> Self;
}

#[derive(Debug)]
pub struct DtorEntry {
    drop: unsafe fn(*mut ()),
    data: *mut (),
}

unsafe impl Send for DtorEntry {}
unsafe impl Sync for DtorEntry {}

impl Drop for DtorEntry {
    fn drop(&mut self) {
        unsafe { (self.drop)(self.data) }
    }
}

impl<T: FromRaw + Send + Sync> From<T> for DtorEntry {
    fn from(value: T) -> Self {
        Self {
            drop: |p| drop(unsafe { T::from_raw(p) }),
            data: value.into_raw(),
        }
    }
}

unsafe impl<T> FromRaw for Arc<T> {
    fn into_raw(self) -> *mut () {
        Arc::into_raw(self).cast_mut().cast()
    }

    unsafe fn from_raw(p: *mut ()) -> Self {
        unsafe { Arc::from_raw(p.cast_const().cast()) }
    }
}

unsafe impl<T> FromRaw for Box<T> {
    fn into_raw(self) -> *mut () {
        Box::into_raw(self).cast()
    }

    unsafe fn from_raw(p: *mut ()) -> Self {
        unsafe { Box::from_raw(p.cast()) }
    }
}
