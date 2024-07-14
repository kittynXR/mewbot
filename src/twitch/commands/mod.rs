pub mod uptime;
pub mod world;
pub mod hello;

pub use uptime::handle_uptime;
pub use world::handle_world;
pub use hello::handle_hello;