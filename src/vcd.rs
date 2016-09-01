use std::io::Write;

use rustc_serialize::{Encoder, Encodable, Decoder, Decodable};
use rustation::tracer::{Tracer, Variable, Event, Value, Collector};

/// Straightforward implementation of a logger storing all events in a
/// `Vec`
pub struct Logger {
    variables: Vec<Variable>,
    /// List of events: (date, id, value). The `id` is simply the
    /// position of the variable in `variables`.
    log: Vec<Event>,
}

impl Default for Logger {
    fn default() -> Self {
        Logger {
            variables: Vec::new(),
            log: Vec::new(),
        }
    }
}

impl Tracer for Logger {
    fn event(&mut self,
             date: u64,
             variable: &str,
             size: u8,
             value: Value) {

        match self.variables.iter().position(|n| n.name() == variable) {
            Some(i) => {
                if self.variables[i].size() != size {
                    panic!("Incoherent size for variable {}: got {} and {}",
                           variable, size, self.variables[i].size());
                }

                self.log.push(Event(date, i as u32, value));
            }
            None => {
                // Add a new variable
                let var = Variable::new(variable.into(), size);

                self.variables.push(var);
                self.log.push(Event(date,
                                    (self.variables.len() - 1) as u32,
                                    value));
            }
        }
    }

    fn variables(&self) -> &[Variable] {
        &self.variables
    }

    fn log(&self) -> &[Event] {
        &self.log
    }

    fn clear(&mut self) {
        self.log.clear();
    }
}

/// Dummy serialization routine for loggers. We want our savestates to
/// remain compatible with and without tracing so it should serialize
/// like `()`. It means that the log itself won't be stored in the
/// savestate, which is probably a good idea...
impl Encodable for Logger {
    fn encode<S>(&self, s: &mut S) -> Result<(), S::Error>
        where S: Encoder {
        s.emit_nil()
    }
}

impl Decodable for Logger {
    fn decode<D>(d: &mut D) -> Result<Logger, D::Error>
        where D: Decoder {
        try!(d.read_nil());

        Ok(Default::default())
    }
}

/// Collector that will dump the collected traces as a VCD file
pub struct Vcd<'a> {
    w: &'a mut Write,
    cur_id: u32,
    // Log of all the events from all modules: (date, variable
    // identifier, value, is_scalar)
    events: Vec<(u64, u32, Value, bool)>,
}

impl<'a> Vcd<'a> {
    pub fn new(w: &'a mut Write) -> Vcd<'a> {
        let mut vcd =
            Vcd {
                w: w,
                cur_id: 0,
                events: Vec::new(),
            };

        vcd.header();

        vcd
    }

    fn header(&mut self) {
        self.write_str("\n");

        // Write the current date
        let now = ::time::now();

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
        self.write_str(&date);

        let version = format!("$version\n  Rustation {}\n$end\n",
                              ::rustation::VERSION);
        self.write_str(&version);

        // For now I hardcode the PSX CPU clock period
        let period_ps = 1_000_000_000_000f64 /
            ::rustation::cpu::CPU_FREQ_HZ as f64;

        let period_ps = period_ps.round() as u32;

        let timescale = format!("$timescale\n  {} ps\n$end\n",
                                period_ps);
        self.write_str(&timescale);

        // Finally we can start with the top level scope
        self.scope("top");
    }

    fn scope(&mut self, name: &str) {
        let scope = format!("$scope module {} $end\n", name);
        self.write_str(&scope);
    }

    fn endscope(&mut self) {
        self.write_str("$upscope $end\n");
    }

    fn write_str(&mut self, s: &str) {
        self.w.write_all(s.as_bytes()).unwrap();
    }
}

impl<'a> Drop for Vcd<'a> {
    fn drop(&mut self) {
        // Finalize header. End top scope.
        self.endscope();

        // Sort all the events by timestamp
        self.events.sort_by_key(|e| e.0);

        self.write_str("#0\n");

        let mut cur_date = 0;

        for &(date, id, val, scalar) in self.events.iter() {
            if date != cur_date {
                self.w.write_all(format!("#{}\n", date).as_bytes()).unwrap();
                cur_date = date;
            }

            let v =
                // Scalars (1bit values) don't have space between the
                // value and identifier
                if scalar {
                    assert!(val & 1 == val);
                    format!("{}{}\n", val, id)
                } else {
                    // Not sure if other formats than binary are supported.
                    format!("b{:b} {}\n", val, id)
                };

            self.w.write_all(v.as_bytes()).unwrap();
        }
    }
}

impl<'a> Collector for Vcd<'a> {
    /// XXX improve error handling...
    type Error = ();

    fn collect<T: Tracer>(&mut self, tracer: &mut T) {
        // For simplicity I simply use a sequential number to identify
        // each variable. VCD format allows for all printable
        // characters (except space obviously) so we could have a more
        // compact representation if we wanted.
        let ids: Vec<_> =
            tracer.variables().iter()
            .map(|v| {
                let id = self.cur_id;
                self.cur_id += 1;

                let var = format!("$var wire {} {} {} $end\n",
                                  v.size(),
                                  id,
                                  v.name());
                self.write_str(&var);

                // Scalars (1bit value) are a special case in the VCD
                // format
                (id, v.size() == 1)
            })
            .collect();

        for &Event(date, module_id, val) in tracer.log().iter() {
            let (id, scalar) = ids[module_id as usize];

            self.events.push((date, id, val, scalar));
        }

        // Finish by clearing the tracer now that we got all its data
        tracer.clear();
    }

    fn submodule<F>(&mut self, name: &str, f: F)
        where F: FnOnce(&mut Self) {
        self.scope(name);
        f(self);
        self.endscope();
    }
}
