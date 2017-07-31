use rustation::cpu::Cpu;
use rustation::memory::map::mask_region;

/// Called every time the PC changes when BIOS call logging is
/// enabled
pub fn check_bios_call(cpu: &mut Cpu) {
    let pc = mask_region(cpu.pc());

    if BIOS_VECTOR_ADDR.contains(&pc) {
        // We're in a BIOS vector call
        let vector = pc;
        // $t1 contains the function number
        let func = cpu.regs()[9];
        let ra = cpu.regs()[31];

        let &(name, param_handlers) = match vector {
            0xa0 => vectors::BIOS_VECTOR_A.get(func as usize),
            0xb0 => vectors::BIOS_VECTOR_B.get(func as usize),
            0xc0 => vectors::BIOS_VECTOR_C.get(func as usize),
            _ => None
        }.unwrap_or(&("unknown", &[]));

        let mut params = String::new();
        let mut first = true;

        for (i, ph) in param_handlers.iter().enumerate() {
            // XXX handle stack parameters when needed
            assert!(i <= 3);

            if first {
                first = false;
            } else {
                params.push_str(", ");
            }

            let reg = cpu.regs()[4 + i];

            params.push_str(&ph(cpu, reg));
        }


        debug!("BIOS call 0x{:02x}[0x{:02x}](RA = 0x{:08x}): {}({})",
               vector, func, ra, name, params);
    }
}

/// The addresses of the three BIOS vectors. In order to call a BIOS
/// function the game sets the function number in R9 before jumping to
/// the function's vector.
const BIOS_VECTOR_ADDR: [u32; 3] = [0xa0, 0xb0, 0xc0];

mod vectors {
    use rustation::cpu::Cpu;
    use rustation::memory::Byte;

    type ParamHandler = fn (&mut Cpu, reg: u32) -> String;

    /// Return true if c is a printable ASCII character
    fn is_printable(c: char) -> bool {
        c >= ' ' && c <= '~'
    }

    fn display_char(c: char) -> String {
        match c {
            '\\' => "\\\\".into(),
            '"' => "\"".into(),
            '\'' => "'".into(),
            _ => {
                if is_printable(c) {
                    format!("{}", c)
                } else {
                    match c {
                        '\n' => "\\n".into(),
                        '\t' => "\\t".into(),
                        _ => format!("\\x{:02x}", c as u8),
                    }
                }
            }
        }
    }

    fn char_t(_cpu: &mut Cpu, reg: u32) -> String {
        let c = reg as u8 as char;

        format!("'{}'", display_char(c))
    }

    fn hex(_cpu: &mut Cpu, reg: u32) -> String {
        format!("0x{:x}", reg)
    }

    fn uint_t(_cpu: &mut Cpu, reg: u32) -> String {
        format!("{}", reg)
    }

    fn size_t(_cpu: &mut Cpu, reg: u32) -> String {
        format!("{}", reg)
    }

    fn int_t(_cpu: &mut Cpu, reg: u32) -> String {
        format!("{}", reg as i32)
    }

    fn ptr(_cpu: &mut Cpu, reg: u32) -> String {
        if reg == 0 {
            "(null)".into()
        } else {
            format!("&0x{:08x}", reg)
        }
    }

    fn func_ptr(_cpu: &mut Cpu, reg: u32) -> String {
        if reg == 0 {
            "(null)()".into()
        } else {
            format!("&0x{:08x}()", reg)
        }
    }

    fn cstr(cpu: &mut Cpu, reg: u32) -> String {

        if reg == 0 {
            return "(null)".into();
        }

        let mut p = reg;
        let mut s = String::new();

        s.push('"');

        /* Limit the size of strings to avoid spamming a huge message for
         * long or buggy strings */
        for _ in 0..32 {
            let b = cpu.examine::<Byte>(p) as u8;

            if b == 0 {
                /* End of string */
                s.push('"');
                return s;
            }

            let c = b as char;

            s.push_str(&display_char(c));
            p = p.wrapping_add(1);
        }

        /* Truncate the string*/
        s.push_str("[...]\"");

        s
    }

    fn void(_cpu: &mut Cpu, _reg: u32) -> String {
        "void".into()
    }


    /// BIOS vector A functions, lifted from No$
    pub static BIOS_VECTOR_A: [(&'static str, &'static [ParamHandler]); 0xb5] = [
        ("FileOpen", &[cstr, hex]),
        ("FileSeek", &[int_t, hex, hex]),
        ("FileRead", &[int_t, ptr, hex]),
        ("FileWrite", &[int_t, cstr, hex]),
        ("FileClose", &[int_t]),
        ("FileIoctl", &[int_t, hex, hex]),
        ("exit", &[uint_t]),
        ("FileGetDeviceFlag", &[int_t]),
        ("FileGetc", &[int_t]),
        ("FilePutc", &[char_t, int_t]),
        ("todigit", &[char_t]),
        ("atof", &[cstr]),
        ("strtoul", &[cstr, ptr, int_t]),
        ("strtol", &[cstr, ptr, int_t]),
        ("abs", &[int_t]),
        ("labs", &[int_t]),
        ("atoi", &[cstr]),
        ("atol", &[cstr]),
        ("atob", &[cstr, ptr]),
        ("SaveState", &[ptr]),
        ("RestoreState", &[ptr, uint_t]),
        ("strcat", &[cstr, cstr]),
        ("strncat", &[cstr, cstr, size_t]),
        ("strcmp", &[cstr, cstr]),
        ("strncmp", &[cstr, cstr, size_t]),
        ("strcpy", &[ptr, cstr]),
        ("strncpy", &[ptr, cstr, size_t]),
        ("strlen", &[cstr]),
        ("index", &[cstr, char_t]),
        ("rindex", &[cstr, char_t]),
        ("strchr", &[cstr, char_t]),
        ("strrchr", &[cstr, char_t]),
        ("strpbrk", &[cstr, ptr]),
        ("strspn", &[cstr, ptr]),
        ("strcspn", &[cstr, ptr]),
        ("strtok", &[cstr, ptr]),
        ("strstr", &[cstr, cstr]),
        ("toupper", &[char_t]),
        ("tolower", &[char_t]),
        ("bcopy", &[ptr, ptr, hex]),
        ("bzero", &[ptr, hex]),
        ("bcmp", &[ptr, ptr, size_t]),
        ("memcpy", &[ptr, ptr, size_t]),
        ("memset", &[ptr, char_t, size_t]),
        ("memmove", &[ptr, ptr, size_t]),
        ("memcmp", &[ptr, ptr, size_t]),
        ("memchr", &[ptr, char_t, size_t]),
        ("rand", &[void]),
        ("srand", &[uint_t]),
        ("qsort", &[ptr, size_t, size_t, func_ptr]),
        ("strtod", &[cstr, ptr]),
        ("malloc", &[size_t]),
        ("free", &[ptr]),
        ("lsearch", &[ptr, ptr, ptr, size_t, func_ptr]),
        ("bsearch", &[ptr, ptr, size_t, size_t, func_ptr]),
        ("calloc", &[size_t, size_t]),
        ("realloc", &[ptr, size_t]),
        ("InitHeap", &[hex, size_t]),
        ("SystemErrorExit", &[uint_t]),
        ("std_in_getchar", &[void]),
        ("std_out_putchar", &[char_t]),
        ("std_in_gets", &[ptr]),
        ("std_out_puts", &[cstr]),
        ("printf", &[cstr]),
        ("SystemErrorUnresolvedException", &[void]),
        ("LoadExeHeader", &[cstr, ptr]),
        ("LoadExeFile", &[cstr, ptr]),
        ("DoExecute", &[ptr, hex, hex]),
        ("FlushCache", &[void]),
        ("init_a0_b0_c0_vectors", &[void]),
        ("GPU_dw", &[uint_t, uint_t, uint_t, uint_t, ptr]),
        ("gpu_send_dma", &[uint_t, uint_t, uint_t, uint_t, ptr]),
        ("SendGP1Command", &[hex]),
        ("GPU_cw", &[hex]),
        ("GPU_cwp", &[ptr, size_t]),
        ("send_gpu_linked_list", &[ptr]),
        ("gpu_abort_dma", &[void]),
        ("GetGPUStatus", &[void]),
        ("gpu_sync", &[void]),
        ("SystemError", &[]),
        ("SystemError", &[]),
        ("LoadAndExecute", &[cstr, hex, hex]),
        ("GetSysSp", &[void]),
        ("SystemError", &[]),
        ("CdInit", &[void]),
        ("_bu_init", &[void]),
        ("CdRemove", &[void]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("dev_tty_init", &[]),
        ("dev_tty_open", &[]),
        ("dev_tty_in_out", &[]),
        ("dev_tty_ioctl", &[]),
        ("dev_cd_open", &[]),
        ("dev_cd_read", &[]),
        ("dev_cd_close", &[]),
        ("dev_cd_firstfile", &[]),
        ("dev_cd_nextfile", &[]),
        ("dev_cd_chdir", &[]),
        ("dev_card_open", &[]),
        ("dev_card_read", &[]),
        ("dev_card_write", &[]),
        ("dev_card_close", &[]),
        ("dev_card_firstfile", &[]),
        ("dev_card_nextfile", &[]),
        ("dev_card_erase", &[]),
        ("dev_card_undelete", &[]),
        ("dev_card_format", &[]),
        ("dev_card_rename", &[]),
        ("unknown", &[]),
        ("_bu_init", &[]),
        ("CdInit", &[]),
        ("CdRemove", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("CdAsyncSeekL", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("CdAsyncGetStatus", &[]),
        ("unknown", &[]),
        ("CdAsyncReadSector", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("CdAsyncSetMode", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("CdromIoIrqFunc1", &[]),
        ("CdromDmaIrqFunc1", &[]),
        ("CdromIoIrqFunc2", &[]),
        ("CdromDmaIrqFunc2", &[]),
        ("CdromGetInt5errCode", &[]),
        ("CdInitSubFunc", &[]),
        ("AddCDROMDevice", &[]),
        ("AddMemCardDevice", &[]),
        ("AddDuartTtyDevice", &[]),
        ("AddDummyTtyDevice", &[]),
        ("SystemError", &[]),
        ("SystemError", &[]),
        ("SetConf", &[]),
        ("GetConf", &[]),
        ("SetCdromIrqAutoAbort", &[]),
        ("SetMemSize", &[]),
        ("WarmBoot", &[]),
        ("SystemErrorBootOrDiskFailure", &[]),
        ("EnqueueCdIntr", &[]),
        ("DequeueCdIntr", &[]),
        ("CdGetLbn", &[]),
        ("CdReadSector", &[]),
        ("CdGetStatus", &[]),
        ("bu_callback_okay", &[]),
        ("bu_callback_err_write", &[]),
        ("bu_callback_err_busy", &[]),
        ("bu_callback_err_eject", &[]),
        ("_card_info", &[]),
        ("_card_async_load_directory", &[]),
        ("set_card_auto_format", &[]),
        ("bu_callback_err_prev_write", &[]),
        ("card_write_test", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("ioabort_raw", &[]),
        ("unknown", &[]),
        ("GetSystemInfo", &[]),
    ];

    /// BIOS vector B functions, lifted from No$
    pub static BIOS_VECTOR_B: [(&'static str, &'static [ParamHandler]); 0x5e] = [
        ("alloc_kernel_memory", &[]),
        ("free_kernel_memory", &[]),
        ("init_timer", &[]),
        ("get_timer", &[]),
        ("enable_timer_irq", &[]),
        ("disable_timer_irq", &[]),
        ("restart_timer", &[]),
        ("DeliverEvent", &[]),
        ("OpenEvent", &[]),
        ("CloseEvent", &[]),
        ("WaitEvent", &[]),
        ("TestEvent", &[]),
        ("EnableEvent", &[]),
        ("DisableEvent", &[]),
        ("OpenThread", &[]),
        ("CloseThread", &[]),
        ("ChangeThread", &[]),
        ("unknown", &[]),
        ("InitPad", &[]),
        ("StartPad", &[]),
        ("StopPad", &[]),
        ("OutdatedPadInitAndStart", &[]),
        ("OutdatedPadGetButtons", &[]),
        ("ReturnFromException", &[]),
        ("SetDefaultExitFromException", &[]),
        ("SetCustomExitFromException", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("UnDeliverEvent", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("FileOpen", &[]),
        ("FileSeek", &[]),
        ("FileRead", &[]),
        ("FileWrite", &[]),
        ("FileClose", &[]),
        ("FileIoctl", &[]),
        ("exit", &[]),
        ("FileGetDeviceFlag", &[]),
        ("FileGetc", &[]),
        ("FilePutc", &[]),
        ("std_in_getchar_t", &[]),
        ("std_out_putchar_t", &[char_t]),
        ("std_in_gets", &[]),
        ("std_out_puts", &[]),
        ("chdir", &[]),
        ("FormatDevice", &[]),
        ("firstfile", &[]),
        ("nextfile", &[]),
        ("FileRename", &[]),
        ("FileDelete", &[]),
        ("FileUndelete", &[]),
        ("AddDevice", &[]),
        ("RemoveDevice", &[]),
        ("PrintInstalledDevices", &[]),
        ("InitCard", &[]),
        ("StartCard", &[]),
        ("StopCard", &[]),
        ("_card_info_subfunc", &[]),
        ("write_card_sector", &[]),
        ("read_card_sector", &[]),
        ("allow_new_card", &[]),
        ("Krom2RawAdd", &[]),
        ("SystemError", &[]),
        ("Krom2Offset", &[]),
        ("GetLastError", &[]),
        ("GetLastFileError", &[]),
        ("GetC0Table", &[]),
        ("GetB0Table", &[]),
        ("get_bu_callback_port", &[]),
        ("testdevice", &[]),
        ("SystemError", &[]),
        ("ChangeClearPad", &[]),
        ("get_card_status", &[]),
        ("wait_card_status", &[]),
    ];

    /// BIOS vector C functions, lifted from No$
    pub static BIOS_VECTOR_C: [(&'static str, &'static [ParamHandler]); 0x1e] = [
        ("EnqueueTimerAndVblankIrqs", &[]),
        ("EnqueueSyscallHandler", &[]),
        ("SysEnqIntRP", &[]),
        ("SysDeqIntRP", &[]),
        ("get_free_EvCB_slot", &[]),
        ("get_free_TCB_slot", &[]),
        ("ExceptionHandler", &[]),
        ("InstallExceptionHandlers", &[]),
        ("SysInitMemory", &[]),
        ("SysInitKernelVariables", &[]),
        ("ChangeClearRCnt", &[]),
        ("SystemError", &[]),
        ("InitDefInt", &[]),
        ("SetIrqAutoAck", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("InstallDevices", &[hex]),
        ("FlushStdInOutPut", &[]),
        ("unknown", &[]),
        ("tty_cdevinput", &[]),
        ("tty_cdevscan", &[]),
        ("tty_circgetc", &[]),
        ("tty_circputc", &[]),
        ("ioabort", &[]),
        ("set_card_find_mode", &[]),
        ("KernelRedirect", &[]),
        ("AdjustA0Table", &[]),
        ("get_card_find_mode", &[]),
    ];
}
