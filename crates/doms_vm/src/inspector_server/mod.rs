//! Chrome DevTools Protocol (CDP) live inspection TCP/WebSocket server.
//!
//! Enables connecting Google Chrome, Edge, and VS Code directly to DOMS:
//! `chrome://inspect` -> targets appear dynamically and support live stepping!

use crate::debugger::CdpDispatcher;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

mod client;
mod server;
mod websocket;

#[cfg(test)]
mod tests;

use client::handle_client;
use websocket::{compute_websocket_accept, read_websocket_frame, write_websocket_frame};

pub use server::InspectorServer;
