use libc::{getrlimit, RLIMIT_NOFILE, RLIMIT_NPROC, rlimit};
use std::mem::MaybeUninit;

fn main() {
    let mut rlim = MaybeUninit::<rlimit>::uninit();
    unsafe {
        if getrlimit(RLIMIT_NOFILE, rlim.as_mut_ptr()) == 0 {
            let res = rlim.assume_init();
            println!("RLIMIT_NOFILE: cur={}, max={}", res.rlim_cur, res.rlim_max);
        }
        if getrlimit(RLIMIT_NPROC, rlim.as_mut_ptr()) == 0 {
            let res = rlim.assume_init();
            println!("RLIMIT_NPROC: cur={}, max={}", res.rlim_cur, res.rlim_max);
        }
    }
}
