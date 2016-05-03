use std::{io, collections};
use time::{get_time, Timespec};

use mio;

use super::message::Message;

pub type EventsConfiguration = mio::EventLoopConfig;

pub type EventLoop = mio::EventLoop<EventsQueue>;

pub struct Timeout {
    pub height: u64,
    pub round: u32,
}

pub struct InternalMessage;

pub enum Event {
    Incoming(Message),
    Internal(InternalMessage),
    Timeout(Timeout),
    Io(mio::Token, mio::EventSet),
    Error(io::Error),
    Terminate
}

pub struct Events {
    event_loop: EventLoop,
    queue: EventsQueue
}

pub struct EventsQueue {
    events: collections::VecDeque<Event>
}

impl EventsQueue {
    fn new() -> EventsQueue {
        EventsQueue {
            // FIXME: configurable capacity?
            events: collections::VecDeque::new(),
        }
    }

    fn push(&mut self, event: Event) {
        self.events.push_back(event)
    }

    fn pop(&mut self) -> Option<Event> {
        self.events.pop_front()
    }
}

impl mio::Handler for EventsQueue {
    type Timeout = Timeout;
    type Message = InternalMessage;

    fn ready(&mut self, _: &mut EventLoop,
             token: mio::Token, events: mio::EventSet) {
        self.push(Event::Io(token, events));
    }

    fn notify(&mut self, _: &mut EventLoop, msg: Self::Message) {
        self.push(Event::Internal(msg));
    }

    fn timeout(&mut self, _: &mut EventLoop, timeout: Self::Timeout) {
        self.push(Event::Timeout(timeout));
    }

    fn interrupted(&mut self, _: &mut EventLoop) {
        self.push(Event::Terminate);
    }
}

impl Events {
    pub fn with_config(config: EventsConfiguration) -> io::Result<Events> {
        // TODO: using EventLoopConfig + capacity of queue
        Ok(Events {
            event_loop: try!(EventLoop::configured(config)),
            queue: EventsQueue::new()
        })
    }

    pub fn poll(&mut self) -> Event {
        loop {
            if let Some(event) = self.queue.pop() {
                return event;
            }
            if let Err(err) = self.event_loop.run_once(&mut self.queue, None) {
                self.queue.push(Event::Error(err))
            }
        }
    }

    pub fn event_loop(&mut self) -> &mut EventLoop {
        &mut self.event_loop
    }

    pub fn push(&mut self, event: Event) {
        self.queue.push(event)
    }

    pub fn add_timeout(&mut self,
                       timeout: Timeout,
                       time: Timespec) {
        let ms = (time - get_time()).num_milliseconds();
        if ms < 0 {
            self.push(Event::Timeout(timeout));
        } else {
            // FIXME: remove unwrap here
            // TODO: use mio::Timeout
            self.event_loop.timeout_ms(timeout, ms as u64).unwrap();
        }
    }
}
