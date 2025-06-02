mod mexc_spot;
mod mexc_swap;
pub mod protobuf;

pub const EXCHANGE_NAME: &str = "mexc";

pub use mexc_spot::{MexcSpotWSClient, MexcUserDataStreamWSClient};
pub use mexc_swap::MexcSwapWSClient;
pub use protobuf::decode_mexc_protobuf;
