#![no_std]
#![no_main]

use core::{alloc::Layout, ffi::*};
use windows_sys::{
    Win32::{Foundation::*, System::LibraryLoader::*},
    core::*,
};

struct SelfAllocator;
unsafe impl core::alloc::GlobalAlloc for SelfAllocator {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { mi_malloc_aligned(layout.size(), layout.align()).cast() }
    }
    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        unsafe { mi_zalloc_aligned(layout.size(), layout.align()).cast() }
    }
    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, _: Layout) {
        unsafe { mi_free(ptr.cast()) }
    }
    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        unsafe { mi_realloc_aligned(ptr.cast(), new_size, layout.align()).cast() }
    }
}

#[global_allocator]
static GLOBAL: SelfAllocator = SelfAllocator; // of course we allocate ourselves

unsafe extern "C" {
    unsafe fn _mi_auto_process_init();
    unsafe fn mi_thread_done();

    unsafe fn mi_any_heap_contains(p: *const c_void) -> bool;

    unsafe fn mi_malloc(n: usize) -> *mut c_void;
    unsafe fn mi_zalloc(n: usize) -> *mut c_void;
    unsafe fn mi_calloc(c: usize, n: usize) -> *mut c_void;
    unsafe fn mi_realloc(p: *mut c_void, n: usize) -> *mut c_void;
    unsafe fn mi_free(p: *mut c_void);

    unsafe fn mi_strdup(s: *const c_char) -> *mut c_char;
    unsafe fn mi_wcsdup(s: *const c_ushort) -> *mut c_ushort;
    unsafe fn mi_dupenv_s(b: *mut *mut c_uchar, s: *mut usize, n: *const c_char) -> c_int;
    unsafe fn mi_wdupenv_s(b: *mut *mut c_ushort, s: *mut usize, n: *const c_ushort) -> c_int;

    unsafe fn mi_expand(p_: *mut c_void, n: usize) -> *mut c_void;
    unsafe fn mi_usable_size(p: *const c_void) -> usize;
    unsafe fn mi_recalloc(p: *mut c_void, c: usize, n: usize) -> *mut c_void;

    unsafe fn mi_malloc_aligned(n: usize, a: usize) -> *mut c_void;
    unsafe fn mi_zalloc_aligned(n: usize, a: usize) -> *mut c_void;
    unsafe fn mi_realloc_aligned(p: *mut c_void, n: usize, a: usize) -> *mut c_void;
    unsafe fn mi_recalloc_aligned(p: *mut c_void, c: usize, n: usize, a: usize) -> *mut c_void;
    unsafe fn mi_malloc_aligned_at(n: usize, a: usize, o: usize) -> *mut c_void;
    unsafe fn mi_zalloc_aligned_at(n: usize, a: usize, o: usize) -> *mut c_void;
    unsafe fn mi_realloc_aligned_at(p: *mut c_void, n: usize, a: usize, o: usize) -> *mut c_void;
    unsafe fn mi_recalloc_aligned_at(
        p: *mut c_void,
        c: usize,
        n: usize,
        a: usize,
        o: usize,
    ) -> *mut c_void;
}

static mut HEAP: HANDLE = core::ptr::null_mut();
windows_link::link!("ntdll.dll" "system" fn RtlFreeHeap(heap: HANDLE, flags: u32, p: *const c_void) -> BOOL);
windows_link::link!("ntdll.dll" "system" fn RtlSizeHeap(heap: HANDLE, flags: u32, p: *const c_void) -> usize);
windows_link::link!("ntdll.dll" "system" fn RtlReAllocateHeap(heap: HANDLE, flags: u32, p: *mut c_void, n: usize) -> *mut c_void);

#[inline(always)]
unsafe fn free_base(p: *const c_void) {
    unsafe { RtlFreeHeap(HEAP, 0, p) };
}

#[inline(always)]
unsafe fn msize_base(p: *const c_void) -> usize {
    unsafe { RtlSizeHeap(HEAP, 0, p) }
}
#[inline]
unsafe fn expand_base(p: *mut c_void, n: usize) -> *mut c_void {
    unsafe {
        let ln = msize_base(p);
        if n > ln {
            core::ptr::null_mut()
        } else if ln <= 0x4000 {
            p
        } else {
            core::hint::cold_path();
            RtlReAllocateHeap(HEAP, 16, p, n)
        }
    }
}

const PTR_SIZE: usize = size_of::<*const c_void>();
#[inline(always)]
fn restore_base(p: *const c_void) -> *const c_void {
    unsafe {
        *(p.map_addr(|p| (p as usize & !(PTR_SIZE - 1)) - PTR_SIZE)
            .cast())
    }
}

#[inline]
unsafe fn custom_aligned_msize(p: *const c_void, a: usize, o: usize) -> usize {
    if likely_stable::likely(!p.is_null()) {
        unsafe {
            msize_base(restore_base(p))
                - (o.wrapping_neg() & (PTR_SIZE - 1))
                - (a.max(PTR_SIZE) - 1)
                - PTR_SIZE
        }
    } else {
        usize::MAX
    }
}

#[inline(always)]
unsafe fn custom_aligned_free(p: *const c_void) {
    if likely_stable::likely(!p.is_null()) {
        unsafe {
            free_base(restore_base(p));
        }
    }
}

macro_rules! boilerplate {
    (fn $name:ident($($pname:ident:$ptype:ty),*) -> $ret:ty
        where mi=$mi:ident($($mi_arg:expr),*), func=$func:ident, check_ptr=$check_expr:expr) => {
        unsafe extern "C" fn $name($($pname:$ptype),*) -> $ret {
            unsafe {
                if likely_stable::likely(mi_any_heap_contains($check_expr)) {
                    $mi($($mi_arg),*)
                } else {
                    core::hint::cold_path();
                    $func($($pname),*)
                }
            }
        }
    };
    (fn $name:ident($($pname:ident:$ptype:ty),*) -> $ret:ty
        where mi=$mi:ident, check_ptr=$check_expr:expr, check($check:expr), size=$size_fn:ident($($size:expr),*),free=$free_fn:ident,
            cold_mi=$cold_mi:ident($cold_count:expr$(,$cold_arg:ident)*)) => {
        unsafe extern "C" fn $name($($pname:$ptype),*) -> $ret {
            unsafe {
                if likely_stable::likely(mi_any_heap_contains($check_expr)) || $check_expr.is_null() {
                    $mi($($pname),*)
                } else {
                    core::hint::cold_path();
                    if likely_stable::unlikely($check) {
                        $free_fn($check_expr);
                        core::ptr::null_mut()
                    } else {
                        match $cold_count {
                            Some(alloc_size) => {
                                let allocated = $cold_mi(alloc_size,$($cold_arg),*);
                                if likely_stable::likely(!allocated.is_null()) {
                                    core::ptr::copy_nonoverlapping($check_expr, allocated, $size_fn($($size),*).min(alloc_size));
                                    $free_fn($check_expr);
                                }
                                allocated
                            }
                            None => core::ptr::null_mut()
                        }
                    }
                }
            }
        }
    };
}

boilerplate! {
    fn mi_sexpand(p: *mut c_void, a: usize) -> *mut c_void
    where mi = mi_expand(p, a), func = expand_base, check_ptr = p
}
boilerplate! {
    fn mi_sfree(p: *mut c_void) -> ()
    where mi = mi_free(p), func = free_base, check_ptr = p
}
boilerplate! {
    fn mi_smsize(p: *const c_void) -> usize
    where mi = mi_usable_size(p), func = msize_base, check_ptr = p
}
boilerplate! {
    fn mi_sfree_aligned(p: *mut c_void) -> ()
    where mi = mi_free(p), func = custom_aligned_free, check_ptr = p
}
boilerplate! {
    fn mi_smsize_aligned(p: *const c_void, a: usize, o: usize) -> usize
    where mi = mi_usable_size(p), func = custom_aligned_msize, check_ptr = p
}

boilerplate! {
    fn mi_srealloc(p: *mut c_void, n: usize) -> *mut c_void
    where mi = mi_realloc, check_ptr = p, check(n == 0), size = msize_base(p), free = free_base, cold_mi = mi_malloc(Some(n))
}
boilerplate! {
    fn mi_srecalloc(p: *mut c_void, c: usize, n: usize) -> *mut c_void
    where mi = mi_recalloc, check_ptr = p, check(n == 0 || c == 0), size = msize_base(p), free = free_base, cold_mi = mi_zalloc(c.checked_mul(n))
}

boilerplate! {
    fn mi_srealloc_aligned(p: *mut c_void, n: usize, a: usize) -> *mut c_void
    where mi = mi_realloc_aligned, check_ptr = p, check(n == 0), size = custom_aligned_msize(p, a, 0), free = custom_aligned_free, cold_mi = mi_malloc_aligned(Some(n), a)
}
boilerplate! {
    fn mi_srecalloc_aligned(p: *mut c_void, c: usize, n: usize, a: usize) -> *mut c_void
    where mi = mi_recalloc_aligned, check_ptr = p, check(n == 0 || c == 0), size = custom_aligned_msize(p, a, 0), free = custom_aligned_free, cold_mi = mi_zalloc_aligned(c.checked_mul(n), a)
}

boilerplate! {
    fn mi_srealloc_aligned_at(p: *mut c_void, n: usize, a: usize, o: usize) -> *mut c_void
    where mi = mi_realloc_aligned_at, check_ptr = p, check(n == 0), size = custom_aligned_msize(p, a, o), free = custom_aligned_free, cold_mi = mi_malloc_aligned_at(Some(n), a, o)
}
boilerplate! {
    fn mi_srecalloc_aligned_at(p: *mut c_void, c: usize, n: usize, a: usize, o: usize) -> *mut c_void
    where mi = mi_recalloc_aligned_at, check_ptr = p, check(n == 0 || c == 0), size = custom_aligned_msize(p, a, o), free = custom_aligned_free, cold_mi = mi_zalloc_aligned_at(c.checked_mul(n), a, o)
}

macro_rules! acquire {
    ($module:expr, $func:literal) => {
        GetProcAddress($module, $func.as_ptr().cast()).unwrap_unchecked() as _
    };
}

#[unsafe(no_mangle)]
unsafe extern "system" fn raw_main(_: HINSTANCE, reason: u32, _: *mut c_void) -> BOOL {
    match reason {
        1 => unsafe {
            _mi_auto_process_init();
            let mut session = neohook::DetourTransaction::begin();
            session.update_all_threads();
            let module = GetModuleHandleW(w!("ucrtbase"));
            HEAP = core::mem::transmute::<*mut (), unsafe extern "C" fn() -> HANDLE>(acquire!(
                module,
                c"_get_heap_handle"
            ))();

            assert!(!module.is_null(), "ucrtbase.dll not found in process");
            session
                .attach(acquire!(module, c"malloc"), mi_malloc as _)
                .unwrap();
            session
                .attach(acquire!(module, c"_o_malloc"), mi_malloc as _)
                .unwrap();
            session
                .attach(acquire!(module, c"calloc"), mi_calloc as _)
                .unwrap();
            session
                .attach(acquire!(module, c"_o_calloc"), mi_calloc as _)
                .unwrap();
            session
                .attach(acquire!(module, c"realloc"), mi_srealloc as _)
                .unwrap();
            session
                .attach(acquire!(module, c"_o_realloc"), mi_srealloc as _)
                .unwrap();
            session
                .attach(acquire!(module, c"free"), mi_sfree as _)
                .unwrap();
            session
                .attach(acquire!(module, c"_o_free"), mi_sfree as _)
                .unwrap();

            session
                .attach(acquire!(module, c"_strdup"), mi_strdup as _)
                .unwrap();
            session
                .attach(acquire!(module, c"_o__strdup"), mi_strdup as _)
                .unwrap();
            session
                .attach(acquire!(module, c"_wcsdup"), mi_wcsdup as _)
                .unwrap();
            session
                .attach(acquire!(module, c"_o__wcsdup"), mi_wcsdup as _)
                .unwrap();

            session
                .attach(acquire!(module, c"_dupenv_s"), mi_dupenv_s as _)
                .unwrap();
            session
                .attach(acquire!(module, c"_o__dupenv_s"), mi_dupenv_s as _)
                .unwrap();
            session
                .attach(acquire!(module, c"_wdupenv_s"), mi_wdupenv_s as _)
                .unwrap();
            session
                .attach(acquire!(module, c"_o__wdupenv_s"), mi_wdupenv_s as _)
                .unwrap();

            session
                .attach(acquire!(module, c"_expand"), mi_sexpand as _)
                .unwrap();
            session
                .attach(acquire!(module, c"_o__expand"), mi_sexpand as _)
                .unwrap();
            session
                .attach(acquire!(module, c"_msize"), mi_smsize as _)
                .unwrap();
            session
                .attach(acquire!(module, c"_o__msize"), mi_smsize as _)
                .unwrap();
            session
                .attach(acquire!(module, c"_recalloc"), mi_srecalloc as _)
                .unwrap();
            session
                .attach(acquire!(module, c"_o__recalloc"), mi_srecalloc as _)
                .unwrap();

            session
                .attach(acquire!(module, c"_aligned_malloc"), mi_malloc_aligned as _)
                .unwrap();
            session
                .attach(
                    acquire!(module, c"_o__aligned_malloc"),
                    mi_malloc_aligned as _,
                )
                .unwrap();
            session
                .attach(
                    acquire!(module, c"_aligned_realloc"),
                    mi_srealloc_aligned as _,
                )
                .unwrap();
            session
                .attach(
                    acquire!(module, c"_o__aligned_realloc"),
                    mi_srealloc_aligned as _,
                )
                .unwrap();
            session
                .attach(
                    acquire!(module, c"_aligned_recalloc"),
                    mi_srecalloc_aligned as _,
                )
                .unwrap();
            session
                .attach(
                    acquire!(module, c"_o__aligned_recalloc"),
                    mi_srecalloc_aligned as _,
                )
                .unwrap();
            session
                .attach(acquire!(module, c"_aligned_msize"), mi_smsize_aligned as _)
                .unwrap();
            session
                .attach(
                    acquire!(module, c"_o__aligned_msize"),
                    mi_smsize_aligned as _,
                )
                .unwrap();
            session
                .attach(acquire!(module, c"_aligned_free"), mi_sfree_aligned as _)
                .unwrap();
            session
                .attach(acquire!(module, c"_o__aligned_free"), mi_sfree_aligned as _)
                .unwrap();

            session
                .attach(
                    acquire!(module, c"_aligned_offset_malloc"),
                    mi_malloc_aligned_at as _,
                )
                .unwrap();
            session
                .attach(
                    acquire!(module, c"_o__aligned_offset_malloc"),
                    mi_malloc_aligned_at as _,
                )
                .unwrap();
            session
                .attach(
                    acquire!(module, c"_aligned_offset_realloc"),
                    mi_srealloc_aligned_at as _,
                )
                .unwrap();
            session
                .attach(
                    acquire!(module, c"_o__aligned_offset_realloc"),
                    mi_srealloc_aligned_at as _,
                )
                .unwrap();
            session
                .attach(
                    acquire!(module, c"_aligned_offset_recalloc"),
                    mi_srecalloc_aligned_at as _,
                )
                .unwrap();
            session
                .attach(
                    acquire!(module, c"_o__aligned_offset_recalloc"),
                    mi_srecalloc_aligned_at as _,
                )
                .unwrap();

            session
                .commit()
                .expect("transaction failed")
                .into_iter()
                .for_each(core::mem::forget);
            let mut _discard = HMODULE::default();
            GetModuleHandleExW(4, raw_main as _, &mut _discard); // GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
        },
        3 => unsafe { mi_thread_done() },
        _ => (),
    }
    TRUE
}
