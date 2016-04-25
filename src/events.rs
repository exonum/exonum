use std::{io, collections};

use mio;

use super::message::Message;

pub type EventsConfiguration = mio::EventLoopConfig;

pub type EventLoop = mio::EventLoop<EventsQueue>;

struct Timeout;
struct InternalMessage;

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

struct EventsQueue {
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

    #[allow(unused_variables)]
    fn ready(&mut self, event_loop: &mut mio::EventLoop<Self>,
             token: mio::Token, events: mio::EventSet) {
        self.push(Event::Io(token, events));
    }

    #[allow(unused_variables)]
    fn notify(&mut self, event_loop: &mut mio::EventLoop<Self>, msg: Self::Message) {
        self.push(Event::Internal(msg));
    }

    #[allow(unused_variables)]
    fn timeout(&mut self, event_loop: &mut mio::EventLoop<Self>, timeout: Self::Timeout) {
        self.push(Event::Timeout(timeout));
    }

    #[allow(unused_variables)]
    fn interrupted(&mut self, event_loop: &mut mio::EventLoop<Self>) {
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
}
