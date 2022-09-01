pub use client::HttpClient;
pub use client::Request;
pub use client::Response;
pub use client::Scheme;
pub use client::Times;

pub use stream::Peer;

mod client;
mod proxy;
mod stream;

#[cfg(test)]
mod test;
