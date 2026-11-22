#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]
#![feature(asm_const)]
#![feature(riscv_ext_intrinsics)]

extern crate alloc;
#[cfg(feature = "axstd")]
extern crate axstd as std;
#[macro_use]
extern crate axlog;

mod csrs;
mod loader;
mod regs;
mod sbi;
mod task;
mod vcpu;

use alloc::boxed::Box;
use axhal::mem::{phys_to_virt, virt_to_phys, PAGE_SIZE_4K};
use axhal::paging::MappingFlags;
use axmm::AddrSpace;
use csrs::defs::hstatus;
use csrs::{RiscvCsrTrait, CSR};
use loader::load_vm_image;
use regs::GprIndex::{self, A0, A1, A7};
use riscv::register::{htval, scause, sstatus, stval};
use sbi::SbiMessage;
use tock_registers::LocalRegisterCopy;
use vcpu::VmCpuRegisters;
use vcpu::_run_guest;

const VM_ENTRY: usize = 0x8020_0000;
const GUEST_HART_ID: usize = 0x1234;
const GUEST_CONFIG_ADDR: usize = 0x40;
const GUEST_CONFIG_VALUE: u64 = 0x6688;

// Sv39x4 uses four physically contiguous pages for its root table.
#[repr(align(16384))]
struct GuestRoot([u8; 4 * PAGE_SIZE_4K]);

#[cfg_attr(feature = "axstd", no_mangle)]
fn main() {
    ax_println!("Hypervisor ...");

    // A new address space for vm.
    let mut uspace = AddrSpace::new_empty(0.into(), 1 << 39).unwrap();

    // Load vm binary file into address space.
    if let Err(e) = load_vm_image("/sbin/skernel2", &mut uspace) {
        panic!("Cannot load app! {:?}", e);
    }

    // Setup context to prepare to enter guest mode.
    let mut ctx = VmCpuRegisters::default();
    prepare_guest_context(&mut ctx);

    // Setup pagetable for 2nd address mapping.
    let mut root = Box::new(GuestRoot([0; 4 * PAGE_SIZE_4K]));
    prepare_vm_pgtable(&mut root, &uspace);
    unsafe { core::arch::asm!("fence.i") };

    // Kick off vm and wait for it to exit.
    loop {
        unsafe { _run_guest(&mut ctx) };
        if vmexit_handler(&mut ctx, &mut uspace, &mut root) {
            break;
        }
    }
    unsafe {
        core::arch::asm!("csrw hgatp, zero");
        core::arch::riscv64::hfence_gvma_all();
    }
}

fn prepare_vm_pgtable(root: &mut GuestRoot, uspace: &AddrSpace) {
    let source = phys_to_virt(uspace.page_table_root());
    let root_addr = virt_to_phys((root.0.as_ptr() as usize).into());
    let hgatp = 8usize << 60 | root_addr.as_usize() >> 12;
    unsafe {
        core::ptr::copy_nonoverlapping(source.as_ptr(), root.0.as_mut_ptr(), PAGE_SIZE_4K);
        core::arch::asm!(
            "csrw hgatp, {hgatp}",
            hgatp = in(reg) hgatp,
        );
        core::arch::riscv64::hfence_gvma_all();
    }
}

fn vmexit_handler(ctx: &mut VmCpuRegisters, uspace: &mut AddrSpace, root: &mut GuestRoot) -> bool {
    use scause::{Exception, Trap};

    let scause = scause::read();
    match scause.cause() {
        Trap::Exception(Exception::VirtualSupervisorEnvCall) => {
            let gprs = &mut ctx.guest_regs.gprs;
            let sbi_msg = SbiMessage::from_regs(gprs.a_regs());
            ax_println!("VmExit Reason: VSuperEcall: {:?}", sbi_msg);
            let legacy_shutdown = gprs.reg(A7) == sbi_spec::legacy::LEGACY_SHUTDOWN;
            if matches!(sbi_msg, Ok(SbiMessage::Reset(_)))
                && (legacy_shutdown || gprs.reg(A0) == sbi_spec::srst::RESET_TYPE_SHUTDOWN as usize)
            {
                if legacy_shutdown {
                    assert_eq!(gprs.reg(A0), GUEST_CONFIG_VALUE as usize);
                    assert_eq!(gprs.reg(A1), GUEST_HART_ID);
                }
                ax_println!("Shutdown vm normally!");
                return true;
            }
            gprs.set_reg(A0, sbi::SBI_ERR_NOT_SUPPORTED as usize);
            gprs.set_reg(A1, 0);
            ctx.guest_regs.sepc += 4;
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            let mut instruction = [0; 4];
            uspace
                .read(ctx.guest_regs.sepc.into(), &mut instruction)
                .unwrap();
            let instruction = u32::from_le_bytes(instruction);
            // CSRRS rd, mhartid, x0 (the csrr pseudoinstruction).
            assert_eq!(
                instruction & !0x0f80,
                0xf140_2073,
                "unsupported guest instruction"
            );
            let rd = GprIndex::from_raw((instruction >> 7) & 0x1f).unwrap();
            ctx.guest_regs.gprs.set_reg(rd, GUEST_HART_ID);
            ctx.guest_regs.sepc += 4;
        }
        Trap::Exception(Exception::LoadGuestPageFault) => {
            let fault_addr = stval::read();
            let guest_paddr = (htval::read() << 2) | (fault_addr & 3);
            assert_eq!(
                guest_paddr, GUEST_CONFIG_ADDR,
                "unexpected guest load fault"
            );
            uspace
                .map_alloc(
                    0.into(),
                    PAGE_SIZE_4K,
                    MappingFlags::READ | MappingFlags::USER,
                    true,
                )
                .unwrap();
            uspace
                .write(GUEST_CONFIG_ADDR.into(), &GUEST_CONFIG_VALUE.to_le_bytes())
                .unwrap();
            prepare_vm_pgtable(root, uspace);
            // Resume at the load so it reads the newly mapped page.
        }
        _ => {
            panic!(
                "Unhandled trap: {:?}, sepc: {:#x}, stval: {:#x}",
                scause.cause(),
                ctx.guest_regs.sepc,
                stval::read()
            );
        }
    }
    false
}

fn prepare_guest_context(ctx: &mut VmCpuRegisters) {
    // Set hstatus
    let mut hstatus =
        LocalRegisterCopy::<usize, hstatus::Register>::new(riscv::register::hstatus::read().bits());
    // Set Guest bit in order to return to guest mode.
    hstatus.modify(hstatus::spv::Guest);
    // Set SPVP bit in order to accessing VS-mode memory from HS-mode.
    hstatus.modify(hstatus::spvp::Supervisor);
    CSR.hstatus.write_value(hstatus.get());
    ctx.guest_regs.hstatus = hstatus.get();

    // Set sstatus in guest mode.
    let mut sstatus = sstatus::read();
    sstatus.set_spp(sstatus::SPP::Supervisor);
    ctx.guest_regs.sstatus = sstatus.bits();
    // Return to entry to start vm.
    ctx.guest_regs.sepc = VM_ENTRY;
}
