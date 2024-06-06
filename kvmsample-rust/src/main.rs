extern crate kvm_ioctls;
extern crate kvm_bindings;
extern crate libc;

use std::{ffi::CString, fs::File, io::{Read, Write}, ptr::null_mut, slice};
use kvm_bindings::{kvm_userspace_memory_region, KVM_MEM_LOG_DIRTY_PAGES};
use kvm_ioctls::{Kvm, VcpuExit, VcpuFd, VmFd};

const RAM_SIZE:i32 =  512000000;
const CODE_START:u16 = 0x1000;
const BASE:u64 = 0x1000 * 16;
const RFLAGS:u64 = 0x0000000000000002;
const RSP:u64 = 0xffffffff;

struct MyKvm {}

impl MyKvm {
    fn kvm_reset_vcpu(vcpu_fd: &VcpuFd) {
        let mut sregs= vcpu_fd.get_sregs().unwrap();
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
        

        vcpu_fd.set_sregs(&sregs).unwrap();

        let mut regs =  vcpu_fd.get_regs().unwrap();
        regs.rflags = RFLAGS;
        regs.rip = 0;
        regs.rsp = RSP;
        regs.rbp = 0;

        vcpu_fd.set_regs(&regs).unwrap();
    }

    fn load_binary(ram_start: *mut u8) {
        const BINARY_FILE:&str = "test.bin";

        match File::open(BINARY_FILE) {
            Ok(mut file) => {
                // let len = file.metadata().unwrap().len();
                let mut buffer: Vec<u8> = Vec::new();
                let len = file.read_to_end(&mut buffer).unwrap();
                unsafe {
                    let mut slice = slice::from_raw_parts_mut(ram_start, len as usize);
                    slice.write(&buffer[..]).unwrap();
                }
            },
            Err(_) => eprintln!("can not open binary file")
        }
    }

    fn kvm_init() -> Kvm {
        const KVM_DEVICE:&str = "/dev/kvm";
        let kvm_path = CString::new(KVM_DEVICE).unwrap();
        Kvm::new_with_path(&kvm_path).unwrap()
    }

    fn kvm_create_vm(kvm: &Kvm, ram_size: i32) -> (VmFd, *mut u8) {
        let vm_fd = Kvm::create_vm(kvm).unwrap();
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
        (vm_fd, ram_start)
    }

    fn kvm_cpu_thread(vcpu_fd: &mut VcpuFd) {
        MyKvm::kvm_reset_vcpu(vcpu_fd);
        loop {
            match vcpu_fd.run().expect("run failed") {
                VcpuExit::IoIn(addr, _data ) => {
                    println!("KVM_EXIT_IO_IN addr {}", addr);
                }
                VcpuExit::IoOut(addr, data) => {
                    println!("KVM_EXIT_IO_OUT addr:{} data:{}", addr, data[0]);
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

    fn kvm_init_vcpu(vm_fd: VmFd, vcpu_id: u64) -> VcpuFd {
        vm_fd.create_vcpu(vcpu_id).unwrap()
    }

    fn kvm_run_vm(mut vcpu_fd: VcpuFd) {
        let handle = std::thread::spawn(move|| {
            MyKvm::kvm_cpu_thread(&mut vcpu_fd);
        });
        handle.join().unwrap();
    }
}

fn main() {
    let kvm = MyKvm::kvm_init();
    let (vm_fd, ram_start) = MyKvm::kvm_create_vm(&kvm, RAM_SIZE);
    MyKvm::load_binary(ram_start);
    let vcpu_fd = MyKvm::kvm_init_vcpu(vm_fd, 0);
    MyKvm::kvm_run_vm(vcpu_fd);
}