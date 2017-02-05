#![allow(unused)]

#![cfg_attr(feature="json", feature(custom_derive, plugin))]
#![cfg_attr(feature="json", plugin(serde_macros))]
#![feature(test)]

#[macro_use]
extern crate lazy_static;
extern crate thread_id;
#[cfg(feature = "json")]
extern crate serde;
#[cfg(feature = "json")]
extern crate serde_json;


extern crate test;

mod html;

use std::cell::{RefCell, Cell};
use std::rc::{Rc, Weak};
use std::iter::Peekable;
use std::borrow::Cow;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use std::collections::BTreeMap;
use std::marker::PhantomData;

pub type SpanPtr = RefCell<Span>;
//lazy_static!(static ref ALL_THREADS: Mutex<Vec<(usize, Option<String>, PrivateFrame)>> = Mutex::new(Vec::new()););

thread_local!(static THREAD_FRAME: RefCell<ThreadFrame> = RefCell::new(ThreadFrame::new()));




struct ThreadFrame {
    epoch: Instant,
    root: Rc<SpanPtr>,
    events: Vec<Event>,
}

struct Event {
    name: &'static str,
    start_ns: u64,
    span: Rc<SpanPtr>,
}

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
        }
    }

    fn start_span(&mut self, name: &'static str) {
        let timestamp = ns_since_epoch(self.epoch);
        let span = Span::sub_span(&self.events[self.events.len()-1].span, name);
        self.events.push(Event::new(name, timestamp, span));
    }
    
    fn end_span(&mut self) {
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
        let mut aspan = parent.children.entry(name)
                                     .or_insert_with(|| Span::new());
        aspan.clone()
    }
}

#[cfg(not(feature="nomock"))]
fn start(name: &'static str) {
    
}

#[cfg(not(feature="nomock"))]
fn end() {
    
}


#[cfg(feature="nomock")]
fn start(name: &'static str) {
    THREAD_FRAME.with(|thread| thread.borrow_mut().start_span(name));
}


#[cfg(feature="nomock")]
fn end() {
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


pub use html::dump_html;