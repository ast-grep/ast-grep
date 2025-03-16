use std::{
  alloc::{self, Layout},
  ffi::{c_char, c_int, c_void},
  mem::align_of,
  ptr,
};
use wasm_bindgen::prelude::*;

/* -------------------------------- stdlib.h -------------------------------- */

#[wasm_bindgen]
extern "C" {
  #[wasm_bindgen(js_namespace = console)]
  fn log(a: &str);
}

/// Allocates memory of the given size.
///
/// # Safety
///
/// The caller must ensure that:
/// - The allocated memory is properly aligned for the intended use
/// - The memory is properly deallocated using `free` when no longer needed
/// - The size doesn't cause integer overflow when calculating the layout
#[no_mangle]
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
  if size == 0 {
    return ptr::null_mut();
  }

  let (layout, offset_to_data) = layout_for_size_prepended(size);
  let buf = alloc::alloc(layout);
  store_layout(buf, layout, offset_to_data)
}

/// Allocates zero-initialized memory for an array of count elements of size bytes each.
///
/// # Safety
///
/// The caller must ensure that:
/// - The allocated memory is properly aligned for the intended use
/// - The memory is properly deallocated using `free` when no longer needed
/// - The size and count don't cause integer overflow when multiplied
#[no_mangle]
pub unsafe extern "C" fn calloc(count: usize, size: usize) -> *mut c_void {
  if count == 0 || size == 0 {
    return ptr::null_mut();
  }

  let (layout, offset_to_data) = layout_for_size_prepended(size * count);
  let buf = alloc::alloc_zeroed(layout);
  store_layout(buf, layout, offset_to_data)
}

/// Reallocates memory to a new size.
///
/// # Safety
///
/// The caller must ensure that:
/// - `buf` is either null or was previously allocated by `malloc`, `calloc`, or `realloc`
/// - The memory is properly deallocated using `free` when no longer needed
/// - The new size doesn't cause integer overflow when calculating the layout
#[no_mangle]
pub unsafe extern "C" fn realloc(buf: *mut c_void, new_size: usize) -> *mut c_void {
  if buf.is_null() {
    malloc(new_size)
  } else if new_size == 0 {
    free(buf);
    ptr::null_mut()
  } else {
    let (old_buf, old_layout) = retrieve_layout(buf);
    let (new_layout, offset_to_data) = layout_for_size_prepended(new_size);
    let new_buf = alloc::realloc(old_buf, old_layout, new_layout.size());
    store_layout(new_buf, new_layout, offset_to_data)
  }
}

/// Deallocates memory previously allocated by `malloc`, `calloc`, or `realloc`.
///
/// # Safety
///
/// The caller must ensure that:
/// - `buf` is either null or was previously allocated by `malloc`, `calloc`, or `realloc`
/// - `buf` has not been previously freed
/// - No references to the memory exist after this call
#[no_mangle]
pub unsafe extern "C" fn free(buf: *mut c_void) {
  if buf.is_null() {
    return;
  }
  let (buf, layout) = retrieve_layout(buf);
  alloc::dealloc(buf, layout);
}

// In all these allocations, we store the layout before the data for later retrieval.
// This is because we need to know the layout when deallocating the memory.
// Here are some helper methods for that:

/// Given a pointer to the data, retrieve the layout and the pointer to the layout.
///
/// # Safety
///
/// The caller must ensure that:
/// - `buf` points to memory previously allocated by our allocation functions
/// - The memory contains a valid `Layout` object at the expected position
unsafe fn retrieve_layout(buf: *mut c_void) -> (*mut u8, Layout) {
  let (_, layout_offset) = Layout::new::<Layout>()
    .extend(Layout::from_size_align(0, align_of::<*const u8>() * 2).unwrap())
    .unwrap();

  let buf = (buf as *mut u8).offset(-(layout_offset as isize));
  let layout = *(buf as *mut Layout);

  (buf, layout)
}

/// Calculate a layout for a given size with space for storing a layout at the start.
/// Returns the layout and the offset to the data.
fn layout_for_size_prepended(size: usize) -> (Layout, usize) {
  Layout::new::<Layout>()
    .extend(Layout::from_size_align(size, align_of::<*const u8>() * 2).unwrap())
    .unwrap()
}

/// Store a layout in the pointer, returning a pointer to where the data should be stored.
///
/// # Safety
///
/// The caller must ensure that:
/// - `buf` points to memory with sufficient space for the layout and the data
/// - `buf` is properly aligned for storing a `Layout`
/// - `offset_to_data` is the correct offset calculated by `layout_for_size_prepended`
unsafe fn store_layout(buf: *mut u8, layout: Layout, offset_to_data: usize) -> *mut c_void {
  *(buf as *mut Layout) = layout;
  buf.add(offset_to_data) as *mut c_void
}
#[no_mangle]
pub unsafe extern "C" fn abort() {
  log("abort");
}

/* -------------------------------- string.h -------------------------------- */

/// Compares at most `n` bytes of two memory regions.
///
/// # Safety
///
/// The caller must ensure that:
/// - Both `ptr1` and `ptr2` point to valid memory regions of at least `n` bytes
/// - The memory regions are properly aligned for reading bytes
#[no_mangle]
pub unsafe extern "C" fn strncmp(ptr1: *const c_void, ptr2: *const c_void, n: usize) -> c_int {
  let s1 = std::slice::from_raw_parts(ptr1 as *const u8, n);
  let s2 = std::slice::from_raw_parts(ptr2 as *const u8, n);

  for (a, b) in s1.iter().zip(s2.iter()) {
    if *a != *b || *a == 0 {
      return (*a as i32) - (*b as i32);
    }
  }

  0
}

/// Calculates the length of a null-terminated string.
///
/// # Safety
///
/// The caller must ensure that:
/// - `s` points to a valid null-terminated C string
/// - The string is valid for reading until a null terminator is found
#[no_mangle]
pub unsafe extern "C" fn strlen(s: *const c_char) -> usize {
  let mut len = 0;
  let mut p = s;
  while *p != 0 {
    len += 1;
    p = p.offset(1);
  }
  len
}

/// Copies at most `n` bytes from `src` to `dest`, stopping after a null byte is encountered.
///
/// # Safety
///
/// The caller must ensure that:
/// - `dest` points to a writable memory region of at least `n` bytes
/// - `src` points to a valid memory region that is readable up to a null terminator or `n` bytes
/// - The memory regions don't overlap in a way that would cause undefined behavior
#[no_mangle]
pub unsafe extern "C" fn strncpy(dest: *mut c_char, src: *const c_char, n: usize) -> *mut c_char {
  let mut i = 0;
  while i < n {
    let c = *src.add(i);
    *dest.add(i) = c;
    i += 1;
    if c == 0 {
      break;
    }
  }

  // Pad with null bytes if necessary
  while i < n {
    *dest.add(i) = 0;
    i += 1;
  }

  dest
}

/// Locates the first occurrence of `c` in the first `n` bytes of the memory region pointed to by `s`.
///
/// # Safety
///
/// The caller must ensure that:
/// - `s` points to a valid memory region of at least `n` bytes
/// - The memory region is properly aligned for reading bytes
#[no_mangle]
pub unsafe extern "C" fn memchr(s: *const c_void, c: c_int, n: usize) -> *mut c_void {
  let bytes = std::slice::from_raw_parts(s as *const u8, n);
  for (i, byte) in bytes.iter().enumerate().take(n) {
    if *byte == c as u8 {
      return (s as *mut u8).add(i) as *mut c_void;
    }
  }
  ptr::null_mut()
}

/// Locates the first occurrence of `c` in the null-terminated string pointed to by `s`.
///
/// # Safety
///
/// The caller must ensure that:
/// - `s` points to a valid null-terminated C string
/// - The string is valid for reading until a null terminator is found
#[no_mangle]
pub unsafe extern "C" fn strchr(s: *const c_char, c: c_int) -> *mut c_char {
  let mut p = s;
  while *p != 0 {
    if *p as c_int == c {
      return p as *mut c_char;
    }
    p = p.offset(1);
  }
  // Also check for null terminator if c is 0
  if c == 0 {
    return p as *mut c_char;
  }
  ptr::null_mut()
}

/// Compares two null-terminated strings.
///
/// # Safety
///
/// The caller must ensure that:
/// - Both `s1` and `s2` point to valid null-terminated C strings
/// - Both strings are valid for reading until their null terminators
#[no_mangle]
pub unsafe extern "C" fn strcmp(s1: *const c_char, s2: *const c_char) -> c_int {
  let mut p1 = s1;
  let mut p2 = s2;

  loop {
    let c1 = *p1 as u8;
    let c2 = *p2 as u8;

    if c1 != c2 {
      return (c1 as c_int) - (c2 as c_int);
    }

    if c1 == 0 {
      return 0;
    }

    p1 = p1.offset(1);
    p2 = p2.offset(1);
  }
}

/* -------------------------------- wctype.h -------------------------------- */

/// Checks if the wide character is a whitespace character.
///
/// # Safety
///
/// The caller must ensure that `c` represents a valid Unicode code point.
#[no_mangle]
pub unsafe extern "C" fn iswspace(c: c_int) -> bool {
  char::from_u32(c as u32).is_some_and(|c| c.is_whitespace())
}

/// Checks if the wide character is alphanumeric.
///
/// # Safety
///
/// The caller must ensure that `c` represents a valid Unicode code point.
#[no_mangle]
pub unsafe extern "C" fn iswalnum(c: c_int) -> bool {
  char::from_u32(c as u32).is_some_and(|c| c.is_alphanumeric())
}

/// Checks if the wide character is a decimal digit.
///
/// # Safety
///
/// The caller must ensure that `c` represents a valid Unicode code point.
#[no_mangle]
pub unsafe extern "C" fn iswdigit(c: c_int) -> bool {
  char::from_u32(c as u32).is_some_and(|c| c.is_ascii_digit())
}

/// Checks if the wide character is a hexadecimal digit.
///
/// # Safety
///
/// The caller must ensure that `c` represents a valid Unicode code point.
#[no_mangle]
pub unsafe extern "C" fn iswxdigit(c: c_int) -> bool {
  char::from_u32(c as u32).is_some_and(|c| c.is_ascii_hexdigit())
}

/// Checks if the wide character is alphabetic.
///
/// # Safety
///
/// The caller must ensure that `c` represents a valid Unicode code point.
#[no_mangle]
pub unsafe extern "C" fn iswalpha(c: c_int) -> bool {
  char::from_u32(c as u32).is_some_and(|c| c.is_alphabetic())
}

/// Converts a wide character to uppercase.
///
/// # Safety
///
/// The caller must ensure that `c` represents a valid Unicode code point.
#[no_mangle]
pub unsafe extern "C" fn towupper(c: c_int) -> c_int {
  char::from_u32(c as u32).map_or(c, |c| c.to_ascii_uppercase() as c_int)
}

/// Checks if the wide character is lowercase.
///
/// # Safety
///
/// The caller must ensure that `c` represents a valid Unicode code point.
#[no_mangle]
pub unsafe extern "C" fn iswlower(c: c_int) -> bool {
  char::from_u32(c as u32).is_some_and(|c| c.is_lowercase())
}

/// Checks if the wide character is uppercase.
///
/// # Safety
///
/// The caller must ensure that `c` represents a valid Unicode code point.
#[no_mangle]
pub unsafe extern "C" fn iswupper(c: c_int) -> bool {
  char::from_u32(c as u32).is_some_and(|c| c.is_uppercase())
}

/* --------------------------------- time.h --------------------------------- */

/// Returns the processor time consumed by the program.
///
/// # Safety
///
/// This function is not supported and will panic when called.
#[no_mangle]
pub unsafe extern "C" fn clock() -> u64 {
  panic!("clock is not supported");
}

/* --------------------------------- ctype.h -------------------------------- */

/// Checks if the character is printable.
///
/// # Safety
///
/// The caller must ensure that `c` is a valid ASCII character code.
#[no_mangle]
pub unsafe extern "C" fn isprint(c: c_int) -> bool {
  (32..=126).contains(&c)
}

/// Checks if the character is a decimal digit.
///
/// # Safety
///
/// The caller must ensure that `c` is a valid ASCII character code.
#[no_mangle]
pub unsafe extern "C" fn isdigit(c: c_int) -> bool {
  (c as u8).is_ascii_digit()
}

/* --------------------------------- stdio.h -------------------------------- */

// #[no_mangle]
// pub unsafe extern "C" fn fprintf(_file: *mut c_void, _format: *const c_void, _args: ...) -> c_int {
//   panic!("fprintf is not supported");
// }

/// Writes a string to a stream.
///
/// # Safety
///
/// This function is not supported and will panic when called.
#[no_mangle]
pub unsafe extern "C" fn fputs(_s: *const c_void, _file: *mut c_void) -> c_int {
  panic!("fputs is not supported");
}

/// Writes a character to a stream.
///
/// # Safety
///
/// This function is not supported and will panic when called.
#[no_mangle]
pub unsafe extern "C" fn fputc(_c: c_int, _file: *mut c_void) -> c_int {
  panic!("fputc is not supported");
}

/// Opens a stream from a file descriptor.
///
/// # Safety
///
/// This function is not supported and will panic when called.
#[no_mangle]
pub unsafe extern "C" fn fdopen(_fd: c_int, _mode: *const c_void) -> *mut c_void {
  panic!("fdopen is not supported");
}

/// Closes a file stream.
///
/// # Safety
///
/// This function is not supported and will panic when called.
#[no_mangle]
pub unsafe extern "C" fn fclose(_file: *mut c_void) -> c_int {
  panic!("fclose is not supported");
}

/// Writes data to a stream.
///
/// # Safety
///
/// This function is not supported and will panic when called.
#[no_mangle]
pub unsafe extern "C" fn fwrite(
  _ptr: *const c_void,
  _size: usize,
  _nmemb: usize,
  _stream: *mut c_void,
) -> usize {
  panic!("fwrite is not supported");
}

// #[no_mangle]
// pub unsafe extern "C" fn vsnprintf(
//   _buf: *mut c_char,
//   _size: usize,
//   _format: *const c_char,
//   _args: ...
// ) -> c_int {
//   panic!("vsnprintf is not supported");
// }

/// Called when an assertion fails.
///
/// # Safety
///
/// This function is not supported and will panic when called.
#[no_mangle]
pub extern "C" fn __assert_fail(_: *const i32, _: *const i32, _: *const i32, _: *const i32) {
  panic!("oh no");
}
