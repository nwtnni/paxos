#![feature(await_macro, async_await, futures_api, pin)]

#[macro_use]
extern crate derivative;

mod config;
mod constants;
mod message;
mod state;
mod shared;
mod thread;
