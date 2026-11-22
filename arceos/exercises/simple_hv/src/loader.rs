use axhal::mem::PAGE_SIZE_4K;
use axhal::paging::MappingFlags;
use axmm::AddrSpace;
use std::fs::File;
use std::io::{self, Read};

use crate::VM_ENTRY;

pub fn load_vm_image(fname: &str, uspace: &mut AddrSpace) -> io::Result<()> {
    let mut file = File::open(fname)?;
    let mut buf = [0; PAGE_SIZE_4K + 1];
    let mut len = 0;
    while len < buf.len() {
        let n = file.read(&mut buf[len..])?;
        if n == 0 {
            break;
        }
        len += n;
    }
    if len == 0 || len > PAGE_SIZE_4K {
        return Err(io::Error::InvalidData);
    }

    uspace.map_alloc(
        VM_ENTRY.into(),
        PAGE_SIZE_4K,
        MappingFlags::READ | MappingFlags::EXECUTE | MappingFlags::USER,
        true,
    )?;
    uspace.write(VM_ENTRY.into(), &buf[..len])?;
    Ok(())
}
