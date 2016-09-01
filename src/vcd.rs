use std::io::Write;

use std::collections::HashMap;
use rustation::tracer::Module;

pub fn dump_trace(w: &mut Write,
                  content: &str,
                  bios: &str,
                  trace: HashMap<&'static str, Module>) {

    write_header(w, content, bios);

    let mut cur_id: u32 = 0;

    let mut log = Vec::new();

    for (name, m) in trace.iter() {
        let v = m.variables();

        if v.is_empty() {
            continue;
        }

        let scope = format!("$scope module {} $end\n", name);
        write_str(w, &scope);

        for (v_name, v) in v.iter() {

            let id = cur_id;
            cur_id += 1;

            let var = format!("$var wire {} {} {} $end\n",
                              v.size(), id, v_name);
            write_str(w, &var);

            // Scalars (1bit values) don't have space between the
            // value and identifier in the VCD dump format
            let is_scalar = v.size() == 1;

            for &(date, value) in v.log() {
                log.push((date, id, is_scalar, value));
            }
        }

        write_str(w, "$upscope $end\n");
    }

    // Sort log by date
    log.sort_by_key(|v| v.0);

    let mut cur_date = 0;

    write_str(w, "#0\n");

    for &(date, id, is_scalar, value) in log.iter() {
        if date != cur_date {
            let d = format!("#{}\n", date);
            write_str(w, &d);
            cur_date = date;
        }

        let v =
            if is_scalar {
                format!("{}{}\n", value, id)
            } else {
                // Apparently only binary is supported...
                format!("b{:b} {}\n", value, id)
            };
        write_str(w, &v);
    }
}

fn write_header(w: &mut Write,
                content: &str,
                bios: &str) {
    // Write the current date
    let now = ::time::now();

    // Replace $ with something else not to configure the VCD
    // parser in the unlikely situation we end up with a VCD
    // directive in a file name
    let content = content.replace('$', " ");
    let bios = bios.replace('$', " ");

    // Put a comment at the top of the file with the content and
    // BIOS information
    let comment = format!("$comment\n  Tracing {}\n  BIOS: {}\n$end\n",
                          content, bios);

    write_str(w, &comment);

    let months = [ "January",    "February",
                    "March",     "April",
                    "May",       "June",
                    "July",      "August",
                    "September", "October",
                    "November",  "December" ];

    let date = format!("$date\n  {} {}, {} {}:{}:{}\n$end\n",
                       months[now.tm_mon as usize],
                       now.tm_mday,
                       now.tm_year + 1900,
                       now.tm_hour,
                       now.tm_min,
                       now.tm_sec);
    write_str(w, &date);

    let version = format!("$version\n  Rustation {}\n$end\n",
                          ::rustation::VERSION);
    write_str(w, &version);

    // For now I hardcode the PSX CPU clock period
    let period_ps = 1_000_000_000_000f64 /
        ::rustation::cpu::CPU_FREQ_HZ as f64;

    let period_ps = period_ps.round() as u32;

    let timescale = format!("$timescale\n  {} ps\n$end\n",
                            period_ps);
    write_str(w, &timescale);
}

fn write_str(w: &mut Write, s: &str) {
    w.write_all(s.as_bytes()).unwrap();
}
