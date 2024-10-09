use event::{user_data, EventFlags, EventQueue};
use libredox::errno::{EAGAIN, EBADF, EWOULDBLOCK};
use libredox::error::Error;
use libredox::{flag, Fd};

use redox_scheme::{CallRequest, RequestKind, Response, SignalBehavior, Socket};
use syscall::EINTR;
use syscall::data::TimeSpec;

mod controlterm;
mod pgrp;
mod pty;
mod resource;
mod scheme;
mod subterm;
mod termios;
mod winsize;

use scheme::PtyScheme;

fn main() {
    redox_daemon::Daemon::new(move |daemon| {
        user_data! {
            enum EventSource {
                Socket,
                Time,
            }
        }

        let event_queue = EventQueue::<EventSource>::new().expect("pty: failed to open event:");

        let time_path = format!("/scheme/time/{}", flag::CLOCK_MONOTONIC);
        let mut time_file =
            Fd::open(&time_path, flag::O_NONBLOCK, 0).expect("pty: failed to open time:");

        let socket =
            redox_scheme::Socket::nonblock("pty").expect("pty: failed to create pty scheme");

        libredox::call::setrens(0, 0).expect("ptyd: failed to enter null namespace");

        event_queue
            .subscribe(socket.inner().raw(), EventSource::Socket, EventFlags::READ)
            .expect("pty: failed to watch events on pty:");
        event_queue
            .subscribe(time_file.raw(), EventSource::Time, EventFlags::READ)
            .expect("pty: failed to watch events on time:");

        println!("ptyd daemon ready");
        daemon.ready().expect("pty: failed to notify parent");

        //TODO: do not set timeout if not necessary
        timeout(&mut time_file).expect("pty: failed to set timeout");

        let mut scheme = PtyScheme::new();
        let mut todo = Vec::new();
        let mut timeout_count = 0u64;

        scan_requests(&socket, &mut scheme, &mut todo).expect("pty: could not scan requests");
        do_todos(&socket, &mut scheme, &mut todo);
        issue_events(&socket, &mut scheme);

        for event_res in event_queue {
            let event = event_res.expect("pty: failed to read from event queue");

            match event.user_data {
                EventSource::Socket => {
                    if scan_requests(&socket, &mut scheme, &mut todo).is_err() {
                        break;
                    }
                }
                EventSource::Time => {
                    timeout(&mut time_file).expect("pty: failed to set timeout");

                    timeout_count = timeout_count.wrapping_add(1);

                    for (_id, handle) in scheme.handles.iter_mut() {
                        handle.timeout(timeout_count);
                    }
                }
            }

            do_todos(&socket, &mut scheme, &mut todo);
            issue_events(&socket, &mut scheme);
        }

        std::process::exit(0);
    })
    .expect("pty: failed to daemonize");
}

struct Todo {
    request: CallRequest,
    cancelling: bool,
}

fn scan_requests(
    socket: &Socket,
    scheme: &mut PtyScheme,
    todo: &mut Vec<Todo>,
) -> libredox::error::Result<()> {
    loop {
        let request = match socket.next_request(SignalBehavior::Restart) {
            Ok(Some(req)) => req,
            Ok(None) => return Err(Error::new(EBADF)),
            Err(error) if error.errno == EWOULDBLOCK || error.errno == EAGAIN => break,
            Err(other) => panic!("pty: failed to read from socket: {other}"),
        };

        match request.kind() {
            RequestKind::Cancellation(req) => {
                if let Some(idx) = todo.iter().position(|t| t.request.request().request_id() == req.id) {
                    todo[idx].cancelling = true;
                }
            }
            RequestKind::Call(request) => {
                if let Some(response) = request.handle_scheme_block_mut(scheme) {
                    let _ = socket
                        .write_response(response, SignalBehavior::Restart)
                        .expect("pty: failed to write responses to pty scheme");
                } else {
                    todo.push(Todo { request, cancelling: false });
                }
            }
            _ => (),
        }
    }
    Ok(())
}

fn do_todos(socket: &Socket, scheme: &mut PtyScheme, todo: &mut Vec<Todo>) {
    let mut i = 0;
    while i < todo.len() {
        if let Some(response) = todo[i].request.handle_scheme_block_mut(scheme) {
            todo.remove(i);
            socket
                .write_response(response, SignalBehavior::Restart)
                .expect("pty: failed to write responses to pty scheme");
        } else if todo[i].cancelling {
            socket.write_response(Response::new(&todo[i].request, Err(Error::new(EINTR).into())), SignalBehavior::Restart)
                .expect("pty: failed to write responses to pty scheme");
            todo.remove(i);
        } else {
            i += 1;
        }
    }
}

fn issue_events(socket: &Socket, scheme: &mut PtyScheme) {
    for (id, handle) in scheme.handles.iter_mut() {
        let events = handle.events();
        if events != syscall::EventFlags::empty() {
            socket
                .post_fevent(*id, events.bits())
                .expect("pty: failed to send scheme event");
        }
    }
}

fn timeout(time_file: &mut Fd) -> libredox::error::Result<()> {
    let mut time = TimeSpec::default();
    time_file.read(&mut time)?;

    time.tv_nsec += 100_000_000;
    while time.tv_nsec >= 1_000_000_000 {
        time.tv_sec += 1;
        time.tv_nsec -= 1_000_000_000;
    }

    time_file.write(&time)?;
    Ok(())
}
