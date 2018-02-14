#![allow(dead_code)]
#![allow(unused_variables)]
#[macro_use]
extern crate lazy_static;
extern crate ctrlc;
pub mod html;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;
use std::collections::BTreeMap;
use std::fs::File;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, Arc};


pub type SpanPtr = RefCell<Span>;
thread_local!(pub static THREAD_FRAME: RefCell<ThreadFrame> = RefCell::new(ThreadFrame::new()));

lazy_static!{
    static ref SETTED_NAME: Mutex<Option<String>> = Mutex::new(None);
    static ref INTERRUPTED_TICKS: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));

}

pub struct ThreadFrame {
    epoch: Instant,
    root: Rc<SpanPtr>,
    events: Vec<Event>,
    dumped_time: usize,
}

struct Event {
    name: &'static str,
    start_ns: u64,
    span: Rc<SpanPtr>,
}

#[derive(Clone)]
pub struct Span {
    value: u64,
    count: u64,
    children: BTreeMap<&'static str, Rc<SpanPtr>>
}

impl Event {
    fn new(name:&'static str, timestamp: u64, span: Rc<SpanPtr>) -> Event {
        Event {
            name: name,
            start_ns: timestamp,
            span: span
        }
    }
}


impl ThreadFrame {

    fn new() -> ThreadFrame {
        let root = Span::new();
        let epoch = Instant::now();
        ThreadFrame {
            epoch: epoch,
            root: root.clone(),
            events: vec![Event::new("Self", ns_since_epoch(epoch), root)],
            dumped_time: 0,
        }
    }

    fn root(&self) -> &SpanPtr {
        &self.root
    }

    fn time_since_start(&self) -> u64 {
        ns_since_epoch(self.epoch)
    }

    fn start_span(&mut self, name: &'static str) {
        let timestamp = ns_since_epoch(self.epoch);
        let span = Span::sub_span(&self.events[self.events.len()-1].span, name);
        self.events.push(Event::new(name, timestamp, span));
    }

    fn end_span(&mut self) {
        let new_time = INTERRUPTED_TICKS.load(Ordering::SeqCst);
        if self.dumped_time < new_time {
            let name: String = SETTED_NAME.lock().unwrap().clone()
                                .expect("Profiler: received interrupt without setted name.");
            File::create(&name)
                    .and_then(|ref mut  file| dump_html(file, &self) )
                    .expect("could not write profiler data");
            self.dumped_time = new_time;
        };

        let timestamp = ns_since_epoch(self.epoch);
        let event = self.events.pop().expect("ThreadFrame::end_span() called events.pop() without a currently running span!");
        let current = event.span;
        // dump self span
        {
            let mut aspan = current.borrow_mut();
            aspan.value += timestamp - event.start_ns;
            aspan.count += 1;
        }
    }

}

fn ns_since_epoch(epoch: Instant) -> u64 {
    let elapsed = epoch.elapsed();
    elapsed.as_secs() * 1000_000_000 + elapsed.subsec_nanos() as u64
}


impl Span {

    fn new() -> Rc<SpanPtr> {
        Rc::new(RefCell::new(Span {
            value: 0,
            count: 0,
            children: BTreeMap::new()
        }))
    }


    fn sub_span(this: &Rc<SpanPtr>, name: &'static str) -> Rc<SpanPtr> {
        let mut parent = this.borrow_mut();
        let  aspan = parent.children.entry(name)
                                     .or_insert_with(|| Span::new());
        aspan.clone()
    }
}

#[cfg(not(feature="nomock"))]
pub fn start(name: &'static str) {

}

#[cfg(not(feature="nomock"))]
pub fn end() {

}


#[cfg(feature="nomock")]
pub fn start(name: &'static str) {
    THREAD_FRAME.with(|thread| thread.borrow_mut().start_span(name));
}


#[cfg(feature="nomock")]
pub fn end() {
    THREAD_FRAME.with(|thread| thread.borrow_mut().end_span());
}


fn spans() -> Rc<SpanPtr> {
    THREAD_FRAME.with(|library| {
        let library = library.borrow();

        let root = library.root.clone();
        root.borrow_mut().value = ns_since_epoch(library.epoch) - library.events[0].start_ns;
        root
    })
}

#[macro_export]
macro_rules! profiler_span{
    ($name:expr, $val:expr) => {
        {
            let _p = $crate::ProfilerSpan::new($name);
            let r = $val;
            drop(_p);
            r
        }
    }
}

#[macro_export]
macro_rules! profiler_next_span{
    ($name:expr) => {
            $crate::end();
            $crate::start($name);
    }
}

pub struct ProfilerSpan<'a> {
    name: &'static str,
    _phantom: PhantomData<&'a ()>
}

impl<'a> Drop for ProfilerSpan<'a> {
    fn drop(&mut self) {
        end();
    }
}

impl<'scope> ProfilerSpan<'scope> {

    pub fn new(name: &'static str) -> ProfilerSpan<'static> {
        start(name);
        ProfilerSpan {
            name: name,
            _phantom:PhantomData
        }
    }

    pub fn next_span(& mut self, name: &'static str) {
        end();
        self.name = name;
        start(self.name);
    }

    pub fn sub_span<'smaller>(&'smaller mut self, name: &'static str) -> ProfilerSpan<'smaller>
    where 'scope: 'smaller {
        ProfilerSpan::new(name)
    }
}

#[cfg(not(feature="nomock"))]
pub fn init_handler(file: String) {
}

#[cfg(feature="nomock")]
pub fn init_handler(file: String) {
    use std::time::{SystemTime, UNIX_EPOCH};

    *SETTED_NAME.lock().unwrap() = Some(file);

    let r = INTERRUPTED_TICKS.clone();
    ::ctrlc::set_handler(move || {
        let secs = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap().as_secs() as usize;
        r.store(secs, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");
}

pub use html::dump_html;
