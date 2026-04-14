#![no_std]
#![feature(const_mut_refs)]
#![feature(strict_provenance)]
#![feature(exposed_provenance)]
#![feature(allocator_api)]
#![feature(generic_const_exprs)]
#![no_main]
#![no_builtins]

extern "C" {
    // Boundaries of the heap
    static mut _sheap: usize;
    static mut _eheap: usize;

    // Boundaries of the stack
    static mut _sstack: usize;
    static mut _estack: usize;
}

core::arch::global_asm!(include_str!("../scripts/asm/asm_reduced.S"));
use rv_bcs::{from_bytes, to_bytes};

#[no_mangle]
extern "C" fn eh_personality() {}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    rust_abort();
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BumpAllocator;

static mut HEAP_NEXT: usize = 0;

#[inline(always)]
const fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

unsafe impl core::alloc::GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let heap_start = core::ptr::addr_of!(_sheap) as usize;
        let heap_end = core::ptr::addr_of!(_eheap) as usize;

        if HEAP_NEXT == 0 {
            HEAP_NEXT = heap_start;
        }

        let alloc_start = align_up(HEAP_NEXT, layout.align());
        let Some(alloc_end) = alloc_start.checked_add(layout.size()) else {
            return core::ptr::null_mut();
        };

        if alloc_end > heap_end {
            return core::ptr::null_mut();
        }

        HEAP_NEXT = alloc_end;
        alloc_start as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {}

    unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: core::alloc::Layout,
        new_size: usize,
    ) -> *mut u8 {
        let Ok(new_layout) = core::alloc::Layout::from_size_align(new_size, layout.align()) else {
            return core::ptr::null_mut();
        };
        let new_ptr = self.alloc(new_layout);
        if new_ptr.is_null() {
            return core::ptr::null_mut();
        }
        core::ptr::copy_nonoverlapping(ptr, new_ptr, core::cmp::min(layout.size(), new_size));
        new_ptr
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR_PLACEHOLDER: BumpAllocator = BumpAllocator;

#[link_section = ".init.rust"]
#[export_name = "_start_rust"]
unsafe extern "C" fn start_rust() -> ! {
    main()
}

#[export_name = "_setup_interrupts"]
pub unsafe fn custom_setup_interrupts() {
    extern "C" {
        fn _machine_start_trap();
    }

    // xtvec::write(_machine_start_trap as *const () as usize, xTrapMode::Direct);
}

#[repr(C)]
#[derive(Debug)]
pub struct MachineTrapFrame {
    pub registers: [u32; 32],
}

/// Exception (trap) handler in rust.
/// Called from the asm/asm.S
#[link_section = ".trap.rust"]
#[export_name = "_machine_start_trap_rust"]
pub extern "C" fn machine_start_trap_rust(_trap_frame: *mut MachineTrapFrame) -> usize {
    0
}

/// Set data as a output of the current execution.
/// By convention, the data that is stored in registers 10-17 after
/// execution has finished is considered 'output' of the computation.
/// Registers 18-25 will be set to 0 as our convention for recursive chain start
#[inline(never)]
pub fn zksync_os_finish_success(data: &[u32; 8]) -> ! {
    let mut result = [0u32; 16];
    result[..8].copy_from_slice(data);
    zksync_os_finish_success_extended(&result)
}

/// Set data as a output of the current execution.
/// By convention, the data that is stored in registers 10-25 after
/// execution has finished is considered 'output' of the computation.
#[inline(never)]
pub fn zksync_os_finish_success_extended(data: &[u32; 16]) -> ! {
    let data_ptr = core::hint::black_box(data.as_ptr().cast::<u32>());
    unsafe {
        core::arch::asm!(
            "lw x10, 0(x26)",
            "lw x11, 4(x26)",
            "lw x12, 8(x26)",
            "lw x13, 12(x26)",
            "lw x14, 16(x26)",
            "lw x15, 20(x26)",
            "lw x16, 24(x26)",
            "lw x17, 28(x26)",
            "lw x18, 32(x26)",
            "lw x19, 36(x26)",
            "lw x20, 40(x26)",
            "lw x21, 44(x26)",
            "lw x22, 48(x26)",
            "lw x23, 52(x26)",
            "lw x24, 56(x26)",
            "lw x25, 60(x26)",
            in("x26") data_ptr,
            out("x10") _,
            out("x11") _,
            out("x12") _,
            out("x13") _,
            out("x14") _,
            out("x15") _,
            out("x16") _,
            out("x17") _,
            out("x18") _,
            out("x19") _,
            out("x20") _,
            out("x21") _,
            out("x22") _,
            out("x23") _,
            out("x24") _,
            out("x25") _,
            options(nostack, preserves_flags)
        )
    }
    loop {
        continue;
    }
}

/// Set data as a output of the current execution. Unsatisfiable in circuits
#[inline(never)]
pub fn zksync_os_finish_error() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[no_mangle]
pub fn rust_abort() -> ! {
    zksync_os_finish_error()
}

#[inline(always)]
/// Writes a given word into CRS register.
pub fn csr_write_word(word: usize) {
    unsafe {
        core::arch::asm!(
            "csrrw x0, 0x7c0, {rd}",
            rd = in(reg) word,
            options(nomem, nostack, preserves_flags)
        )
    }
}

/// QuasiUART start marker recognized by the simulator host logger.
const QUASI_UART_HELLO: u32 = u32::MAX;

/// Send a log line to host console using QuasiUART framing on CSR 0x7c0.
fn guest_log(msg: &str) {
    let bytes = msg.as_bytes();
    let len = bytes.len();
    csr_write_word(QUASI_UART_HELLO as usize);
    csr_write_word(len.next_multiple_of(4) / 4 + 1);
    csr_write_word(len);

    let mut i = 0usize;
    while i < len {
        let mut chunk = [0u8; 4];
        let end = (i + 4).min(len);
        chunk[..end - i].copy_from_slice(&bytes[i..end]);
        csr_write_word(u32::from_le_bytes(chunk) as usize);
        i = end;
    }
}

#[inline(always)]
pub fn csr_read_word() -> u32 {
    let mut output;
    unsafe {
        core::arch::asm!(
            "csrrw {rd}, 0x7c0, x0",
            rd = out(reg) output,
            options(nomem, nostack, preserves_flags)
        );
    }

    output
}

#[inline(always)]
const fn to_hex_ascii(nibble: u8) -> u8 {
    match nibble {
        0..=9 => b'0' + nibble,
        _ => b'a' + (nibble - 10),
    }
}

#[inline(always)]
fn encode_hex_for_output(bytes: &[u8]) -> [u32; 16] {
    let mut out = [0u32; 16];
    // Word 0 stores original byte length.
    out[0] = bytes.len() as u32;

    // Remaining 15 words store up to 30 input bytes as 60 ASCII hex chars.
    let mut hex_index = 0usize;
    for byte in bytes.iter().take(30) {
        let hi = to_hex_ascii(byte >> 4);
        let lo = to_hex_ascii(byte & 0x0f);
        for ascii in [hi, lo] {
            let word_idx = 1 + (hex_index / 4);
            let shift = (hex_index % 4) * 8;
            out[word_idx] |= (ascii as u32) << shift;
            hex_index += 1;
        }
    }

    out
}

#[inline(always)]
fn log_bytes_via_csr(bytes: &[u8]) {
    let mut hex_buf = [0u8; 128];
    let max_bytes = core::cmp::min(bytes.len(), hex_buf.len() / 2);
    for (i, byte) in bytes.iter().take(max_bytes).enumerate() {
        hex_buf[2 * i] = to_hex_ascii(byte >> 4);
        hex_buf[2 * i + 1] = to_hex_ascii(byte & 0x0f);
    }

    guest_log("[rv-bcs] encoded_bytes(hex):");
    if let Ok(hex_str) = core::str::from_utf8(&hex_buf[..2 * max_bytes]) {
        guest_log(hex_str);
    } else {
        guest_log("[rv-bcs] failed to format hex log");
    }

    if max_bytes < bytes.len() {
        guest_log("[rv-bcs] encoded_bytes(hex) truncated");
    }
}

unsafe fn workload() -> ! {
    // let a: [u32; 16] = core::array::from_fn(|_| csr_read_word());
    // let b: [u32; 16] = core::array::from_fn(|_| csr_read_word());

    let some_data: Option<u8> = Some(8);
    let encoded_bytes = match to_bytes(&some_data) {
        Ok(bytes) => bytes,
        Err(_) => zksync_os_finish_error(),
    };
    log_bytes_via_csr(&encoded_bytes);
    let output_words = encode_hex_for_output(&encoded_bytes);
    zksync_os_finish_success_extended(&output_words);
}

#[inline(never)]
fn main() -> ! {
    unsafe { workload() }
}