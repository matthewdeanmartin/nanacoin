#![no_std]
extern "C" {
    fn NCBANK(operation: *mut u8, frame: *mut u8, result: *mut u8) -> i32;
}

// Minimal framework call through the same fixed-width C linkage as Windows.
// Runtime initialization and serialization are the caller's responsibility.
#[no_mangle]
pub unsafe extern "C" fn nc_rust_probe() -> i32 {
    let mut operation: i32 = 1;
    let mut frame = [0i64; 64];
    frame[0] = 100;
    frame[2] = 10;
    let mut result: i32 = -1;
    NCBANK(
        (&mut operation as *mut i32).cast(),
        frame.as_mut_ptr().cast(),
        (&mut result as *mut i32).cast(),
    );
    if result != 0 || frame[6] != 90 || frame[7] != 10 { return -1; }
    operation = 2;
    frame = [0; 64];
    frame[0] = 1;
    frame[1] = 8;
    frame[2] = i64::MAX;
    NCBANK((&mut operation as *mut i32).cast(), frame.as_mut_ptr().cast(),
           (&mut result as *mut i32).cast());
    if result != 0 || frame[3] != 100_000_000 { return -2; }
    operation = 6;
    frame = [0; 64];
    frame[..19].copy_from_slice(&[1,1,200,1700086400,200,0,10000,0,0,
                                 1700000000,1700086400,0,0,0,10000,100,1,1,1000]);
    NCBANK((&mut operation as *mut i32).cast(), frame.as_mut_ptr().cast(),
           (&mut result as *mut i32).cast());
    if result != 0 || frame[21] != 9900 || frame[30] != 100 || frame[31] != 100 { return -3; }
    0
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }
