use std::ffi::CString;
use std::os::raw::{c_char, c_void};
use std::ptr;
use std::slice;

#[repr(C)]
struct CTensorView {
    data_ptr: *const u8,
    byte_size: u64,
    dtype_code: u8,
    shape_ptr: *const i64,
    shape_len: usize,
    internal_view: *mut c_void,
}

extern "C" {
    fn nexus_st_init(file_path: *const c_char) -> *mut c_void;
    fn nexus_st_get(handle: *mut c_void, name: *const c_char, out_view: *mut CTensorView) -> bool;
    fn nexus_st_free_view(handle: *mut c_void, view: *mut CTensorView);
    fn nexus_st_deinit(handle: *mut c_void);
}

pub struct EdgeLoader {
    handle: *mut c_void,
}

impl EdgeLoader {
    pub fn new(path: &str) -> Result<Self, &'static str> {
        let c_path = CString::new(path).map_err(|_| "path contained interior nul")?;
        let handle = unsafe { nexus_st_init(c_path.as_ptr()) };
        if handle.is_null() {
            return Err("nexus_st_init failed");
        }
        Ok(Self { handle })
    }

    pub fn get_tensor<'a>(&'a self, name: &str) -> Result<TensorSlice<'a>, &'static str> {
        let c_name = CString::new(name).map_err(|_| "tensor name contained interior nul")?;
        let mut c_view = CTensorView {
            data_ptr: ptr::null(),
            byte_size: 0,
            dtype_code: 0,
            shape_ptr: ptr::null(),
            shape_len: 0,
            internal_view: ptr::null_mut(),
        };

        let ok = unsafe { nexus_st_get(self.handle, c_name.as_ptr(), &mut c_view) };
        if !ok || c_view.data_ptr.is_null() {
            return Err("tensor not found");
        }

        // Read the fields we need BEFORE moving c_view
        let data = unsafe { slice::from_raw_parts(c_view.data_ptr, c_view.byte_size as usize) };
        let shape = unsafe { slice::from_raw_parts(c_view.shape_ptr, c_view.shape_len) };
        let dtype = c_view.dtype_code;

        Ok(TensorSlice {
            engine: self,
            c_view,          // move happens here
            data,
            shape,
            dtype,           // already copied
        })
    }
}

impl Drop for EdgeLoader {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { nexus_st_deinit(self.handle) };
        }
    }
}

pub struct TensorSlice<'a> {
    engine: &'a EdgeLoader,
    c_view: CTensorView,
    pub data: &'a [u8],
    pub shape: &'a [i64],
    pub dtype: u8,
}

impl<'a> Drop for TensorSlice<'a> {
    fn drop(&mut self) {
        unsafe { nexus_st_free_view(self.engine.handle, &mut self.c_view) };
    }
}

impl<'a> TensorSlice<'a> {
    pub fn as_bf16(&self) -> Result<TypedTensor<'a, half::bf16>, &'static str> {
        if self.dtype != 3 {
            return Err("not BF16");
        }
        if !self.data.len().is_multiple_of(2) {
            return Err("odd length");
        }
        let ptr = self.data.as_ptr() as *const half::bf16;
        let len = self.data.len() / 2;
        let data = unsafe { slice::from_raw_parts(ptr, len) };
        Ok(TypedTensor { data, shape: self.shape })
    }
}

pub struct TypedTensor<'a, T> {
    pub data: &'a [T],
    pub shape: &'a [i64],
}
