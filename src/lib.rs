#![no_std]
#![no_main]

use core::{
    alloc::Layout,
    ffi::{c_char, c_int, c_uchar, c_ushort, c_void},
    sync::atomic::{AtomicPtr, Ordering},
};
use windows_sys::{
    Win32::{
        Foundation::*,
        System::LibraryLoader::{GetModuleHandleExW, GetModuleHandleW, GetProcAddress},
    },
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

    unsafe fn mi_zalloc_aligned(n: usize, a: usize) -> *mut c_void;

    unsafe fn mi_zalloc_aligned_at(n: usize, a: usize, o: usize) -> *mut c_void;

    unsafe fn mi_malloc(n: usize) -> *mut c_void;
    unsafe fn mi_zalloc(n: usize) -> *mut c_void;
    unsafe fn mi_calloc(c: usize, n: usize) -> *mut c_void;
    unsafe fn mi_realloc(p: *mut c_void, n: usize) -> *mut c_void;
    unsafe fn mi_free(p: *mut c_void);

    unsafe fn mi_strdup(s: *const c_char) -> *mut c_char;
    unsafe fn mi_wcsdup(s: *const c_ushort) -> *mut c_ushort;
    unsafe fn mi_mbsdup(s: *const c_uchar) -> *mut c_uchar;
    unsafe fn mi_dupenv_s(b: *mut *mut c_uchar, s: *mut usize, n: *const c_char) -> c_int;
    unsafe fn mi_wdupenv_s(b: *mut *mut c_ushort, s: *mut usize, n: *const c_ushort) -> c_int;

    unsafe fn mi_expand(p_: *mut c_void, n: usize) -> *mut c_void;
    unsafe fn mi_usable_size(p: *const c_void) -> usize;
    unsafe fn mi_recalloc(p: *mut c_void, c: usize, n: usize) -> *mut c_void;

    unsafe fn mi_malloc_aligned(n: usize, a: usize) -> *mut c_void;
    unsafe fn mi_realloc_aligned(p: *mut c_void, n: usize, a: usize) -> *mut c_void;
    unsafe fn mi_recalloc_aligned(p: *mut c_void, c: usize, n: usize, a: usize) -> *mut c_void;
    unsafe fn mi_malloc_aligned_at(n: usize, a: usize, o: usize) -> *mut c_void;
    unsafe fn mi_realloc_aligned_at(p: *mut c_void, n: usize, a: usize, o: usize) -> *mut c_void;
    unsafe fn mi_recalloc_aligned_at(
        p: *mut c_void,
        c: usize,
        n: usize,
        a: usize,
        o: usize,
    ) -> *mut c_void;
}

macro_rules! call_fn {
    ($holder:ident, $fn:ty) => {
        core::mem::transmute::<*mut _, $fn>($holder.load(Ordering::Relaxed))
    };
}

type FreeFn = unsafe extern "C" fn(*mut c_void);
macro_rules! boilerplate {
    (fn $name:ident($($pname:ident:$ptype:ty),*) -> $ret:ty
        where mi=$mi:ident($($mi_arg:expr),*), holder=$holder:ident, check_ptr=$check_expr:expr) => {
        static $holder: AtomicPtr<u8> = AtomicPtr::new(core::ptr::null_mut());
        #[inline(always)]
        unsafe extern "C" fn $name($($pname:$ptype),*) -> $ret {
            type Fn = unsafe extern "C" fn($($ptype),*) -> $ret;
            unsafe {
                if mi_any_heap_contains($check_expr) {
                    $mi($($mi_arg),*)
                } else {
                    core::hint::cold_path();
                    call_fn!($holder, Fn)($($pname),*)
                }
            }
        }
    };
    (fn $name:ident($($pname:ident:$ptype:ty),*) -> $ret:ty
        where mi=$mi:ident, check_ptr=$check_expr:expr, check($check:expr), size=$size_fn:ident($($size:expr),*)as$size_ty:ty,free=$free_fn:ident,
            cold_mi=$cold_mi:ident($cold_count:expr$(,$cold_arg:expr)*)) => {
        #[inline(always)]
        unsafe extern "C" fn $name($($pname:$ptype),*) -> $ret {
            unsafe {
                if mi_any_heap_contains($check_expr) || $check_expr.is_null() {
                    $mi($($pname),*)
                } else {
                    core::hint::cold_path();
                    if $check {
                        call_fn!($free_fn, FreeFn)($check_expr);
                        core::ptr::null_mut()
                    } else {
                        let old_size = call_fn!($size_fn, $size_ty)($($size),*);
                        let new_size = $cold_count;
                        let to = $cold_mi(new_size,$($cold_arg),*);
                        core::ptr::copy_nonoverlapping($check_expr, to, old_size.min(new_size));
                        call_fn!($free_fn, FreeFn)($check_expr);
                        to
                    }
                }
            }
        }
    };
}

type SizeFn = unsafe extern "C" fn(*const c_void) -> usize;
type AlignedSizeFn = unsafe extern "C" fn(*const c_void, usize, usize) -> usize;

boilerplate! {
    fn mi_sexpand(p: *mut c_void, a: usize) -> *mut c_void
    where mi = mi_expand(p, a), holder = EXPAND, check_ptr = p
}
boilerplate! {
    fn mi_sfree(p: *mut c_void) -> ()
    where mi = mi_free(p), holder = FREE, check_ptr = p
}
boilerplate! {
    fn mi_sfree_aligned(p: *mut c_void) -> ()
    where mi = mi_free(p), holder = ALIGNED_FREE, check_ptr = p
}
boilerplate! {
    fn mi_smsize(p: *const c_void) -> usize
    where mi = mi_usable_size(p), holder = MSIZE, check_ptr = p
}
boilerplate! {
    fn mi_smsize_aligned(p: *const c_void, a: usize, o: usize) -> usize
    where mi = mi_usable_size(p), holder = ALIGNED_MSIZE, check_ptr = p
}

boilerplate! {
    fn mi_srealloc(p: *mut c_void, n: usize) -> *mut c_void
    where mi = mi_realloc, check_ptr = p, check(n == 0), size = MSIZE(p) as SizeFn, free = FREE, cold_mi = mi_malloc(n)
}
boilerplate! {
    fn mi_srecalloc(p: *mut c_void, c: usize, n: usize) -> *mut c_void
    where mi = mi_recalloc, check_ptr = p, check(n == 0 || c == 0), size = MSIZE(p) as SizeFn, free = FREE, cold_mi = mi_zalloc(c * n)
}

boilerplate! {
    fn mi_srealloc_aligned(p: *mut c_void, n: usize, a: usize) -> *mut c_void
    where mi = mi_realloc_aligned, check_ptr = p, check(n == 0), size = ALIGNED_MSIZE(p, a, 0) as AlignedSizeFn, free = ALIGNED_FREE, cold_mi = mi_malloc_aligned(n, a)
}
boilerplate! {
    fn mi_srecalloc_aligned(p: *mut c_void, c: usize, n: usize, a: usize) -> *mut c_void
    where mi = mi_recalloc_aligned, check_ptr = p, check(n == 0 || c == 0), size = ALIGNED_MSIZE(p, a, 0) as AlignedSizeFn, free = ALIGNED_FREE, cold_mi = mi_zalloc_aligned(c * n, a)
}

boilerplate! {
    fn mi_srealloc_aligned_at(p: *mut c_void, n: usize, a: usize, o: usize) -> *mut c_void
    where mi = mi_realloc_aligned_at, check_ptr = p, check(n == 0), size = ALIGNED_MSIZE(p, a, o) as AlignedSizeFn, free = ALIGNED_FREE, cold_mi = mi_malloc_aligned_at(n, a, o)
}
boilerplate! {
    fn mi_srecalloc_aligned_at(p: *mut c_void, c: usize, n: usize, a: usize, o: usize) -> *mut c_void
    where mi = mi_recalloc_aligned_at, check_ptr = p, check(n == 0 || c == 0), size = ALIGNED_MSIZE(p, a, o) as AlignedSizeFn, free = ALIGNED_FREE, cold_mi = mi_zalloc_aligned_at(c * n, a, o)
}

macro_rules! hook {
    ($module:expr, $func:literal) => {
        GetProcAddress($module, $func.as_ptr().cast()).unwrap_unchecked() as _
    };
}

#[unsafe(no_mangle)]
unsafe extern "system" fn raw_main(_: HMODULE, reason: u32, _: *mut c_void) -> BOOL {
    match reason {
        1 => unsafe {
            _mi_auto_process_init();
            let mut session = neohook::DetourTransaction::begin();
            session.update_all_threads();
            let module = GetModuleHandleW(w!("ucrtbase"));
            assert!(!module.is_null(), "ucrtbase.dll not found in process");
            session
                .attach(hook!(module, c"malloc"), mi_malloc as _)
                .unwrap();
            session
                .attach(hook!(module, c"calloc"), mi_calloc as _)
                .unwrap();
            session
                .attach(hook!(module, c"realloc"), mi_srealloc as _)
                .unwrap();
            session
                .attach(hook!(module, c"free"), mi_sfree as _)
                .unwrap();
            FREE.store(hook!(module, c"_free_base"), Ordering::Relaxed);

            session
                .attach(hook!(module, c"_strdup"), mi_strdup as _)
                .unwrap();
            session
                .attach(hook!(module, c"_wcsdup"), mi_wcsdup as _)
                .unwrap();
            session
                .attach(hook!(module, c"_mbsdup"), mi_mbsdup as _)
                .unwrap();
            session
                .attach(hook!(module, c"_dupenv_s"), mi_dupenv_s as _)
                .unwrap();
            session
                .attach(hook!(module, c"_wdupenv_s"), mi_wdupenv_s as _)
                .unwrap();

            EXPAND.store(
                session
                    .attach(hook!(module, c"_expand"), mi_sexpand as _)
                    .unwrap(),
                Ordering::Relaxed,
            );
            MSIZE.store(
                session
                    .attach(hook!(module, c"_msize"), mi_smsize as _)
                    .unwrap(),
                Ordering::Relaxed,
            );
            session
                .attach(hook!(module, c"_recalloc"), mi_srecalloc as _)
                .unwrap();

            session
                .attach(hook!(module, c"_aligned_malloc"), mi_malloc_aligned as _)
                .unwrap();
            session
                .attach(hook!(module, c"_aligned_realloc"), mi_srealloc_aligned as _)
                .unwrap();
            session
                .attach(
                    hook!(module, c"_aligned_recalloc"),
                    mi_srecalloc_aligned as _,
                )
                .unwrap();
            ALIGNED_MSIZE.store(
                session
                    .attach(hook!(module, c"_aligned_msize"), mi_smsize_aligned as _)
                    .unwrap(),
                Ordering::Relaxed,
            );
            ALIGNED_FREE.store(
                session
                    .attach(hook!(module, c"_aligned_free"), mi_sfree_aligned as _)
                    .unwrap(),
                Ordering::Relaxed,
            );
            session
                .attach(
                    hook!(module, c"_aligned_offset_malloc"),
                    mi_malloc_aligned_at as _,
                )
                .unwrap();
            session
                .attach(
                    hook!(module, c"_aligned_offset_realloc"),
                    mi_srealloc_aligned_at as _,
                )
                .unwrap();
            session
                .attach(
                    hook!(module, c"_aligned_offset_recalloc"),
                    mi_srecalloc_aligned_at as _,
                )
                .unwrap();
            core::mem::forget(session.commit().expect("transaction failed"));
            let mut _discard = HMODULE::default();
            GetModuleHandleExW(4, raw_main as _, &mut _discard); // GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
        },
        3 => unsafe { mi_thread_done() },
        _ => (),
    }
    TRUE
}
