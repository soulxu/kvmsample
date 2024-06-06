extern crate kvm_ioctls;
extern crate kvm_bindings;
extern crate libc;

use std::{ffi::CString, fs::File, io::{Read, Write}, ptr::null_mut, slice};
use kvm_bindings::{kvm_userspace_memory_region, KVM_MEM_LOG_DIRTY_PAGES};
use kvm_ioctls::{Kvm, VcpuExit, VcpuFd, VmFd};
// use libc;

const RAM_SIZE:i32 =  512000000;
const CODE_START:u16 = 0x1000;
const BASE:u64 = 0x1000 * 16;
const RFLAGS:u64 = 0x0000000000000002;
const RSP:u64 = 0xffffffff;

struct MyKvm {
    kvm: Kvm,
    kvm_version: i32,
    vm_fd: VmFd,
    vcpu_fd: VcpuFd,
    ram_size: u64,
    ram_start: *mut u8,
    mem: kvm_userspace_memory_region,
}

impl MyKvm {
    fn kvm_reset_vcpu(&self) {
        let mut sregs= self.vcpu_fd.get_sregs().unwrap();
        sregs.cs.selector = CODE_START;
        sregs.cs.base = BASE;
        sregs.ss.selector = CODE_START;
        sregs.ss.base = BASE;
        sregs.ds.selector = CODE_START;
        sregs.ds.base = BASE;
        sregs.es.selector = CODE_START;
        sregs.es.base = BASE;
        sregs.fs.selector = CODE_START;
        sregs.fs.base = BASE;
        sregs.gs.selector = CODE_START;
        

        self.vcpu_fd.set_sregs(&sregs);

        let mut regs =  self.vcpu_fd.get_regs().unwrap();
        regs.rflags = RFLAGS;
        regs.rip = 0;
        regs.rsp = RSP;
        regs.rbp = 0;

        self.vcpu_fd.set_regs(&regs);
    }

    fn load_binary(&self) {
        const BINARY_FILE:&str = "test.bin";

        match File::open(BINARY_FILE) {
            Ok(mut file) => {
                // let len = file.metadata().unwrap().len();
                let mut buffer: Vec<u8> = Vec::new();
                let len = file.read_to_end(&mut buffer).unwrap();
                unsafe {
                    let mut slice = slice::from_raw_parts_mut(self.ram_start, len as usize);
                    slice.write(&buffer[..]).unwrap();
                }
            },
            Err(_) => eprintln!("can not open binary file")
        }
    }

    fn kvm_init() -> MyKvm {
        const KVM_DEVICE:&str = "/dev/kvm";
        let kvm_path = CString::new(KVM_DEVICE).unwrap();
        let mut my_kvm = MyKvm {
            kvm: Kvm::new_with_path(&kvm_path).unwrap(),
            ..Default::default()
        };
        my_kvm.kvm_version = Kvm::get_api_version(&my_kvm.kvm);
        my_kvm
    }

    fn kvm_create_vm(&mut self, ram_size: i32) -> i32 {
        let vm_fd = Kvm::create_vm(&self.kvm).unwrap();
        let ram_size = ram_size as u64;
        let ram_start = unsafe {
            libc::mmap(
                null_mut(),
                ram_size as usize,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_ANONYMOUS | libc::MAP_SHARED | libc::MAP_NORESERVE,
                -1,
                0
            ) as *mut u8
        };

        // if self.ram_start == libc::MAP_FAILED {
        //     eprintln!("can not mmap ram");
        //     return -1
        // }
        let mem = kvm_userspace_memory_region {
            slot: 0,
            flags: KVM_MEM_LOG_DIRTY_PAGES,
            guest_phys_addr: 0,
            memory_size: ram_size,
            userspace_addr: ram_start as u64
        };

        unsafe {
            vm_fd.set_user_memory_region(mem).unwrap();
        }
        0
    }

    fn kvm_cpu_thread(&mut self) {
        MyKvm::kvm_reset_vcpu(self);
        loop {
            match self.vcpu_fd.run().expect("run failed") {
                VcpuExit::IoIn(addr, _data ) => {
                    println!("KVM_EXIT_IO_IN addr{}", addr);
                }
                VcpuExit::IoOut(addr, _data) => {
                    println!("KVM_EXIT_IO_OUT addr{}", addr);
                }
                VcpuExit::Unknown => {
                    println!("KVM_EXIT_UNKNOWN");
                }
                VcpuExit::Debug(_debug) => {
                    println!("KVM_EXIT_DEBUG");
                }
                VcpuExit::MmioRead(_addr, _data ) => {
                    println!("KVM_EXIT_MMIO_READ");
                }
                VcpuExit::MmioWrite(_addr, _data ) => {
                    println!("KVM_EXIT_MMIO_WRITE");
                }
                VcpuExit::Intr => {
                    println!("KVM_EXIT_INTR");
                }
                VcpuExit::Shutdown => {
                    println!("KVM_EXIT_SHUTDOWN");
                    break;
                }
                r => panic!("KVM PANIC {:?}", r)
            }
        }
    }

    fn kvm_init_vcpu(&mut self, vcpu_id: u64) {
        self.vcpu_fd = self.vm_fd.create_vcpu(vcpu_id).unwrap();
    }

    fn kvm_run_vm(&mut self) {
        self.kvm_cpu_thread();
    }

}

fn main() {
    let mut my_kvm = MyKvm::kvm_init();
    my_kvm.kvm_create_vm(RAM_SIZE);
    my_kvm.load_binary();
    my_kvm.kvm_init_vcpu(0);
    my_kvm.kvm_run_vm();
}
