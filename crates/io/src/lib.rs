pub mod aedat4_decoder;
pub mod codec;
pub mod error;
pub mod file;

#[path = "../generated/aedat4_header_generated.rs"]
#[allow(unsafe_code)]
pub mod aedat4_header_generated;

#[path = "../generated/aedat4_data_generated.rs"]
#[allow(unsafe_code)]
pub mod aedat4_data_generated;

#[path = "../generated/event_generated.rs"]
#[allow(unsafe_code)]
pub mod event;

#[path = "../generated/frame_generated.rs"]
#[allow(unsafe_code)]
pub mod frame;

#[path = "../generated/imu_generated.rs"]
#[allow(unsafe_code)]
pub mod imu;

#[path = "../generated/trigger_generated.rs"]
#[allow(unsafe_code)]
pub mod trigger;
