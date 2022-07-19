extern crate redox_termios;
extern crate syscall;

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;

use syscall::data::{Event, Packet, TimeSpec};
use syscall::flag::{CloneFlags, EventFlags};
use syscall::scheme::SchemeBlockMut;

mod master;
mod pgrp;
mod pty;
mod resource;
mod scheme;
mod slave;
mod termios;
mod winsize;

use scheme::PtyScheme;

fn main(){
    redox_daemon::Daemon::new(move |daemon| {
        let mut event_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open("event:")
            .expect("pty: failed to open event:");

        let time_path = format!("time:{}", syscall::CLOCK_MONOTONIC);
        let mut time_file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(syscall::O_NONBLOCK as i32)
            .open(time_path)
            .expect("pty: failed to open time:");

        let mut socket = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .custom_flags(syscall::O_NONBLOCK as i32)
            .open(":pty")
            .expect("pty: failed to create pty scheme");

        syscall::setrens(0, 0).expect("ptyd: failed to enter null namespace");

        daemon.ready().expect("pty: failed to notify parent");

        event_file.write(&Event {
            id: socket.as_raw_fd() as usize,
            flags: syscall::EVENT_READ,
            data: 1,
        }).expect("pty: failed to watch events on pty:");

        event_file.write(&Event {
            id: time_file.as_raw_fd() as usize,
            flags: syscall::EVENT_READ,
            data: 2,
        }).expect("pty: failed to watch events on time:");

        //TODO: do not set timeout if not necessary
        timeout(&mut time_file)
            .expect("pty: failed to set timeout");

        let mut scheme = PtyScheme::new();
        let mut todo = Vec::new();
        let mut timeout_count = 0u64;
        loop {
            let mut event = Event::default();
            event_file.read(&mut event)
                .expect("pty: failed to read event:");

            match event.data {
                1 => {
                    let mut packet = Packet::default();
                    socket.read(&mut packet).expect("pty: failed to read events from pty scheme");

                    if let Some(a) = scheme.handle(&mut packet) {
                        packet.a = a;
                        socket.write(&packet).expect("pty: failed to write responses to pty scheme");
                    } else {
                        todo.push(packet);
                    }
                },
                2 => {
                    timeout(&mut time_file)
                        .expect("pty: failed to set timeout");

                    timeout_count.wrapping_add(1);

                    for (_id, handle) in scheme.handles.iter_mut() {
                        handle.timeout(timeout_count);
                    }
                }
                _ => (),
            }

            let mut i = 0;
            while i < todo.len() {
                if let Some(a) = scheme.handle(&mut todo[i]) {
                    let mut packet = todo.remove(i);
                    packet.a = a;
                    socket.write(&packet).expect("pty: failed to write responses to pty scheme");
                } else {
                    i += 1;
                }
            }

            for (id, handle) in scheme.handles.iter_mut() {
                let events = handle.events();
                if events != EventFlags::empty() {
                    post_fevent(&mut socket, *id, events, 1);
                }
            }
        }
    }).expect("pty: failed to daemonize");
}

fn timeout(time_file: &mut File) -> io::Result<()> {
    let mut time = TimeSpec::default();
    time_file.read_exact(&mut time)?;

    time.tv_nsec += 100_000_000;
    while time.tv_nsec >= 1_000_000_000 {
        time.tv_sec += 1;
        time.tv_nsec -= 1_000_000_000;
    }

    time_file.write_all(&time)
}

fn post_fevent(socket: &mut File, id: usize, flags: EventFlags, count: usize) {
    socket.write(&Packet {
        id: 0,
        pid: 0,
        uid: 0,
        gid: 0,
        a: syscall::number::SYS_FEVENT,
        b: id,
        c: flags.bits(),
        d: count
    }).expect("pty: failed to write event");
}
