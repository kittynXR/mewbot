mod uptime;
mod world;
mod hello;
mod ping;

pub use uptime::handle_uptime;
pub use world::handle_world;
pub use hello::handle_hello;
pub use ping::handle_ping;