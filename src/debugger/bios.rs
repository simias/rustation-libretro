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

    /// Return true if c is a printable ASCII character (including
    /// whitespace)
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

    fn event_class(_cpu: &mut Cpu, reg: u32) -> String {
        let c =
            match reg {
                0...0xf => format!("MC:{:x}", reg),
                0xf0000001 => "VBLANK IRQ".into(),
                0xf0000002 => "GPU IRQ1".into(),
                0xf0000003 => "CDROM IRQ".into(),
                0xf0000004 => "DMA IRQ".into(),
                0xf0000005 => "TIMER0 IRQ".into(),
                0xf0000006 => "TIMER1/2 IRQ".into(),
                0xf0000008 => "PADMEMCARD IRQ".into(),
                0xf0000009 => "SPU IRQ".into(),
                0xf000000A => "PIO IRQ".into(),
                0xf000000B => "SIO IRQ".into(),
                0xf0000010 => "Exception!".into(),
                0xf0000011 => "MEMCARD event 1".into(),
                0xf0000012 => "MEMCARD event 2".into(),
                0xf0000013 => "MEMCARD event 3".into(),

                0xf2000000 => "PCLK counter ".into(),
                0xf2000001 => "HBLANK counter".into(),
                0xf2000002 => "SYSCLK/8 counter".into(),
                0xf2000003 => "VBLANK counter".into(),

                0xf3000000...0xf3ffffff =>
                    format!("User event 0x{:x}", reg & 0xffffff),

                0xf4000001 => "MEMCARD BIOS event".into(),
                0xf4000002 => "Libmath event".into(),

                0xff000000...0xffffffff =>
                    format!("Thread event 0x{:x}", reg & 0xffffff),

                _ =>
                    format!("UNKNOWN EVENT 0x{:x}", reg)
            };

        format!("Class {} [0x{:x}]", c, reg)
    }

    fn event_spec(_cpu: &mut Cpu, reg: u32) -> String {
        
        // XXX This looks like it should be a bitfield but I'm not
        // sure that it is? In particular codes like 0x8001 make no
        // sense.
        let spec =
            match reg {
                0x0001 => "Counter is 0",
                0x0002 => "Interrupt",
                0x0004 => "End of I/O",
                0x0008 => "File closed",
                0x0010 => "Command acknowledged",
                0x0020 => "Command completed",
                0x0040 => "Data ready",
                0x0080 => "Data end",
                0x0100 => "Timeout",
                0x0200 => "Unknown command",
                0x0400 => "End of read buffer",
                0x0800 => "End of write buffer",
                0x1000 => "General interrupt",
                0x2000 => "New device",
                0x4000 => "System call",
                0x8000 => "Error",
                0x8001 => "Write error",
                0x0301 => "Libmath domain error",
                0x0302 => "Libmath range error",
                _      => "Unknown",
            };

        format!("Spec {} [0x{:x}]", spec, reg)
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
        ("dev_tty_init", &[void]),
        ("dev_tty_open", &[uint_t, cstr, hex]),
        ("dev_tty_in_out", &[uint_t, hex]),
        ("dev_tty_ioctl", &[uint_t, hex, hex]),
        ("dev_cd_open", &[uint_t, cstr, hex]),
        ("dev_cd_read", &[uint_t, ptr, size_t]),
        ("dev_cd_close", &[uint_t]),
        ("dev_cd_firstfile", &[uint_t, cstr, hex]),
        ("dev_cd_nextfile", &[uint_t, uint_t]),
        ("dev_cd_chdir", &[uint_t, cstr]),
        ("dev_card_open", &[uint_t, cstr, hex]),
        ("dev_card_read", &[uint_t, ptr, size_t]),
        ("dev_card_write", &[uint_t, ptr, size_t]),
        ("dev_card_close", &[uint_t]),
        ("dev_card_firstfile", &[uint_t, cstr, hex]),
        ("dev_card_nextfile", &[uint_t, uint_t]),
        ("dev_card_erase", &[uint_t, cstr]),
        ("dev_card_undelete", &[uint_t, cstr]),
        ("dev_card_format", &[uint_t]),
        ("dev_card_rename", &[uint_t, cstr, uint_t, cstr]),
        ("unknown", &[]),
        ("_bu_init", &[void]),
        ("CdInit", &[void]),
        ("CdRemove", &[void]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("CdAsyncSeekL", &[ptr]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("CdAsyncGetStatus", &[ptr]),
        ("unknown", &[]),
        ("CdAsyncReadSector", &[uint_t, ptr, hex]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("CdAsyncSetMode", &[hex]),
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
        ("CdromIoIrqFunc1", &[void]),
        ("CdromDmaIrqFunc1", &[void]),
        ("CdromIoIrqFunc2", &[void]),
        ("CdromDmaIrqFunc2", &[void]),
        ("CdromGetInt5errCode", &[ptr, ptr]),
        ("CdInitSubFunc", &[void]),
        ("AddCDROMDevice", &[void]),
        ("AddMemCardDevice", &[void]),
        ("AddDuartTtyDevice", &[void]),
        ("AddDummyTtyDevice", &[void]),
        ("SystemError", &[]),
        ("SystemError", &[]),
        ("SetConf", &[uint_t, uint_t, ptr]),
        ("GetConf", &[ptr, ptr, ptr]),
        ("SetCdromIrqAutoAbort", &[uint_t, hex]),
        ("SetMemSize", &[uint_t]),
        ("WarmBoot", &[void]),
        ("SystemErrorBootOrDiskFailure", &[cstr, hex]),
        ("EnqueueCdIntr", &[void]),
        ("DequeueCdIntr", &[void]),
        ("CdGetLbn", &[cstr]),
        ("CdReadSector", &[size_t, uint_t, ptr]),
        ("CdGetStatus", &[void]),
        ("bu_callback_okay", &[]),
        ("bu_callback_err_write", &[]),
        ("bu_callback_err_busy", &[]),
        ("bu_callback_err_eject", &[]),
        ("_card_info", &[uint_t]),
        ("_card_async_load_directory", &[uint_t]),
        ("set_card_auto_format", &[hex]),
        ("bu_callback_err_prev_write", &[]),
        ("card_write_test", &[uint_t]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("ioabort_raw", &[uint_t]),
        ("unknown", &[]),
        ("GetSystemInfo", &[hex]),
    ];

    /// BIOS vector B functions, lifted from No$
    pub static BIOS_VECTOR_B: [(&'static str, &'static [ParamHandler]); 0x5e] = [
        ("alloc_kernel_memory", &[size_t]),
        ("free_kernel_memory", &[ptr]),
        ("init_timer", &[uint_t, hex, hex]),
        ("get_timer", &[uint_t]),
        ("enable_timer_irq", &[uint_t]),
        ("disable_timer_irq", &[uint_t]),
        ("restart_timer", &[uint_t]),
        ("DeliverEvent", &[event_class, event_spec]),
        ("OpenEvent", &[event_class, event_spec, hex, func_ptr]),
        ("CloseEvent", &[uint_t]),
        ("WaitEvent", &[uint_t]),
        ("TestEvent", &[uint_t]),
        ("EnableEvent", &[uint_t]),
        ("DisableEvent", &[uint_t]),
        ("OpenThread", &[ptr, ptr, ptr]),
        ("CloseThread", &[ptr]),
        ("ChangeThread", &[ptr]),
        ("unknown", &[]),
        ("InitPad", &[ptr, size_t, ptr, size_t]),
        ("StartPad", &[void]),
        ("StopPad", &[void]),
        ("OutdatedPadInitAndStart", &[hex, ptr, hex, hex]),
        ("OutdatedPadGetButtons", &[void]),
        ("ReturnFromException", &[void]),
        ("SetDefaultExitFromException", &[void]),
        ("SetCustomExitFromException", &[ptr]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("unknown", &[]),
        ("UnDeliverEvent", &[event_class, event_spec]),
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
        ("FileOpen", &[cstr, hex]),
        ("FileSeek", &[uint_t, size_t, hex]),
        ("FileRead", &[uint_t, ptr, size_t]),
        ("FileWrite", &[uint_t, ptr, size_t]),
        ("FileClose", &[uint_t]),
        ("FileIoctl", &[uint_t, hex, hex]),
        ("exit", &[uint_t]),
        ("FileGetDeviceFlag", &[uint_t]),
        ("FileGetc", &[uint_t]),
        ("FilePutc", &[char_t, uint_t]),
        ("std_in_getchar", &[void]),
        ("std_out_putchar", &[char_t]),
        ("std_in_gets", &[ptr]),
        ("std_out_puts", &[ptr]),
        ("chdir", &[cstr]),
        ("FormatDevice", &[cstr]),
        ("firstfile", &[cstr, hex]),
        ("nextfile", &[cstr, hex]),
        ("FileRename", &[cstr, cstr]),
        ("FileDelete", &[cstr]),
        ("FileUndelete", &[cstr]),
        ("AddDevice", &[ptr]),
        ("RemoveDevice", &[cstr]),
        ("PrintInstalledDevices", &[void]),
        ("InitCard", &[hex]),
        ("StartCard", &[void]),
        ("StopCard", &[void]),
        ("_card_info_subfunc", &[uint_t]),
        ("write_card_sector", &[uint_t, uint_t, ptr]),
        ("read_card_sector", &[uint_t, uint_t, ptr]),
        ("allow_new_card", &[void]),
        ("Krom2RawAdd", &[hex]),
        ("SystemError", &[]),
        ("Krom2Offset", &[hex]),
        ("GetLastError", &[void]),
        ("GetLastFileError", &[uint_t]),
        ("GetC0Table", &[void]),
        ("GetB0Table", &[void]),
        ("get_bu_callback_port", &[void]),
        ("testdevice", &[cstr]),
        ("SystemError", &[]),
        ("ChangeClearPad", &[uint_t]),
        ("get_card_status", &[uint_t]),
        ("wait_card_status", &[uint_t]),
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
