#![no_std]

use core::{ffi::c_void, mem::transmute};
use windows_link::link;
use windows_sys::{
    Win32::{
        Foundation::*,
        System::LibraryLoader::{
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GetModuleHandleExW, GetModuleHandleW,
            GetProcAddress,
        },
    },
    core::*,
};

use core::alloc::Layout;
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

    unsafe fn mi_any_heap_contains(p: *const c_void) -> bool;

    unsafe fn mi_zalloc_aligned(n: usize, a: usize) -> *mut c_void;

    unsafe fn mi_malloc(n: usize) -> *mut c_void;
    unsafe fn mi_calloc(c: usize, n: usize) -> *mut c_void;
    unsafe fn mi_realloc(p: *mut c_void, n: usize) -> *mut c_void;
    unsafe fn mi_free(p: *mut c_void);

    unsafe fn mi_expand(p_: *mut c_void, n: usize) -> *mut c_void;
    unsafe fn mi_usable_size(p: *const c_void) -> usize;
    unsafe fn mi_recalloc(p: *mut c_void, b: usize, n: usize) -> *mut c_void;

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

macro_rules! boilerplate {
    ($name:ident, $mi:ident, $holder:ident) => {
        static mut $holder: *mut u8 = core::ptr::null_mut();
        #[inline(always)]
        unsafe extern "C" fn $name(p: *mut c_void) {
            type Fn = unsafe extern "C" fn(*mut c_void);
            unsafe {
                if mi_any_heap_contains(p) {
                    $mi(p)
                } else {
                    core::hint::cold_path();
                    transmute::<*mut _, Fn>($holder)(p)
                }
            }
        }
    };
    ($name:ident, $mi:ident, $holder:ident, 0) => {
        static mut $holder: *mut u8 = core::ptr::null_mut();
        #[inline(always)]
        unsafe extern "C" fn $name(p: *const c_void) -> usize {
            type Fn = unsafe extern "C" fn(*const c_void) -> usize;
            unsafe {
                if mi_any_heap_contains(p) {
                    $mi(p)
                } else {
                    core::hint::cold_path();
                    transmute::<*mut _, Fn>($holder)(p)
                }
            }
        }
    };
    ($name:ident, $mi:ident, $holder:ident, -2) => {
        static mut $holder: *mut u8 = core::ptr::null_mut();
        #[inline(always)]
        unsafe extern "C" fn $name(p: *const c_void, a: usize, b: usize) -> usize {
            type Fn = unsafe extern "C" fn(*const c_void, usize, usize) -> usize;
            unsafe {
                if mi_any_heap_contains(p) {
                    $mi(p)
                } else {
                    core::hint::cold_path();
                    transmute::<*mut _, Fn>($holder)(p, a, b)
                }
            }
        }
    };
    ($name:ident, $mi:ident, $holder:ident, 1) => {
        static mut $holder: *mut u8 = core::ptr::null_mut();
        #[inline(always)]
        unsafe extern "C" fn $name(p: *mut c_void, a: usize) -> *mut c_void {
            type Fn = unsafe extern "C" fn(*mut c_void, usize) -> *mut c_void;
            unsafe {
                if mi_any_heap_contains(p) {
                    $mi(p, a)
                } else {
                    core::hint::cold_path();
                    transmute::<*mut _, Fn>($holder)(p, a)
                }
            }
        }
    };
    ($name:ident, $mi:ident, $holder:ident, 2) => {
        static mut $holder: *mut u8 = core::ptr::null_mut();
        #[inline(always)]
        unsafe extern "C" fn $name(p: *mut c_void, a: usize, b: usize) -> *mut c_void {
            type Fn = unsafe extern "C" fn(*mut c_void, usize, usize) -> *mut c_void;
            unsafe {
                if mi_any_heap_contains(p) {
                    $mi(p, a, b)
                } else {
                    core::hint::cold_path();
                    transmute::<*mut _, Fn>($holder)(p, a, b)
                }
            }
        }
    };
    ($name:ident, $mi:ident, $holder:ident, 3) => {
        static mut $holder: *mut u8 = core::ptr::null_mut();
        #[inline(always)]
        unsafe extern "C" fn $name(p: *mut c_void, a: usize, b: usize, c: usize) -> *mut c_void {
            type Fn = unsafe extern "C" fn(*mut c_void, usize, usize, usize) -> *mut c_void;
            unsafe {
                if mi_any_heap_contains(p) {
                    $mi(p, a, b, c)
                } else {
                    core::hint::cold_path();
                    transmute::<*mut _, Fn>($holder)(p, a, b, c)
                }
            }
        }
    };
    ($name:ident, $mi:ident, $holder:ident, 4) => {
        static mut $holder: *mut u8 = core::ptr::null_mut();
        #[inline(always)]
        unsafe extern "C" fn $name(
            p: *mut c_void,
            a: usize,
            b: usize,
            c: usize,
            d: usize,
        ) -> *mut c_void {
            unsafe {
                type Fn =
                    unsafe extern "C" fn(*mut c_void, usize, usize, usize, usize) -> *mut c_void;
                if mi_any_heap_contains(p) {
                    $mi(p, a, b, c, d)
                } else {
                    core::hint::cold_path();
                    transmute::<*mut _, Fn>($holder)(p, a, b, c, d)
                }
            }
        }
    };
}

#[inline(always)]
unsafe extern "C" fn mi_sfree(p: *mut c_void) {
    link!("ucrtbase.dll" "C" fn _free_base(p : *mut c_void));
    unsafe {
        if mi_any_heap_contains(p) {
            mi_free(p)
        } else {
            core::hint::cold_path();
            _free_base(p)
        }
    }
}

#[inline(always)]
unsafe extern "C" fn mi_srealloc(p: *mut c_void, n: usize) -> *mut c_void {
    link!("ucrtbase.dll" "C" fn _realloc_base(p: *mut c_void, n: usize) -> *mut c_void);
    unsafe {
        if mi_any_heap_contains(p) {
            mi_realloc(p, n)
        } else {
            core::hint::cold_path();
            _realloc_base(p, n)
        }
    }
}

boilerplate!(mi_sexpand, mi_expand, EXPAND, 1);
boilerplate!(mi_smsize, mi_usable_size, MSIZE, 0);
boilerplate!(mi_srecalloc, mi_recalloc, RECALLOC, 2);

boilerplate!(mi_srealloc_aligned, mi_realloc_aligned, ALIGNED_REALLOC, 2);
boilerplate!(
    mi_srecalloc_aligned,
    mi_recalloc_aligned,
    ALIGNED_RECALLOC,
    3
);
boilerplate!(mi_smsize_aligned, mi_usable_size, ALIGNED_MSIZE, -2);
boilerplate!(mi_sfree_aligned, mi_free, ALIGNED_FREE);
boilerplate!(
    mi_srealloc_aligned_at,
    mi_realloc_aligned_at,
    ALIGNED_REALLOC_OFFSET,
    3
);
boilerplate!(
    mi_srecalloc_aligned_at,
    mi_recalloc_aligned_at,
    ALIGNED_RECALLOC_OFFSET,
    4
);

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

            EXPAND = session
                .attach(hook!(module, c"_expand"), mi_sexpand as _)
                .unwrap();
            MSIZE = session
                .attach(hook!(module, c"_msize"), mi_smsize as _)
                .unwrap();
            RECALLOC = session
                .attach(hook!(module, c"_recalloc"), mi_srecalloc as _)
                .unwrap();

            session
                .attach(hook!(module, c"_aligned_malloc"), mi_malloc_aligned as _)
                .unwrap();
            ALIGNED_REALLOC = session
                .attach(hook!(module, c"_aligned_realloc"), mi_srealloc_aligned as _)
                .unwrap();
            ALIGNED_RECALLOC = session
                .attach(
                    hook!(module, c"_aligned_recalloc"),
                    mi_srecalloc_aligned as _,
                )
                .unwrap();
            ALIGNED_MSIZE = session
                .attach(hook!(module, c"_aligned_msize"), mi_smsize_aligned as _)
                .unwrap();
            ALIGNED_FREE = session
                .attach(hook!(module, c"_aligned_free"), mi_sfree_aligned as _)
                .unwrap();
            session
                .attach(
                    hook!(module, c"_aligned_offset_malloc"),
                    mi_malloc_aligned_at as _,
                )
                .unwrap();
            ALIGNED_REALLOC_OFFSET = session
                .attach(
                    hook!(module, c"_aligned_offset_realloc"),
                    mi_srealloc_aligned_at as _,
                )
                .unwrap();
            ALIGNED_RECALLOC_OFFSET = session
                .attach(
                    hook!(module, c"_aligned_offset_recalloc"),
                    mi_srecalloc_aligned_at as _,
                )
                .unwrap();
            core::mem::forget(session.commit().expect("transaction failed"));
            let mut _discard = HMODULE::default();
            GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
                raw_main as _,
                &mut _discard,
            )
        },
        _ => TRUE,
    }
}
