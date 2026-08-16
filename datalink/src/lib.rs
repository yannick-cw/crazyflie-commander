pub mod downlink {
    include!(concat!(env!("OUT_DIR"), "/downlink.rs"));
}

pub mod compression_adapter;
pub mod domain_types;
