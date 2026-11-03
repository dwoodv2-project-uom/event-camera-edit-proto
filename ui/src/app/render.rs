use crate::visualisations::time_surface::DecayMode;

/// Provides information needed to plug into the render function of a certain visualisation
/// E.g. a time surface needs timestamp of last polarity on event
pub struct RenderCursor {
    pub width: u32,
    pub height: u32,
    pub timestamp: u64,
}

pub enum VisualisationMode {
    TimeSurface,
}

pub enum VisualisationParams {
    TimeSurfaceParams,
}

pub struct TimeSurfaceParams {
    pub decay_constant: f64,
    pub decay_mode: DecayMode,
}

pub struct VisualisationState {
    pub mode: VisualisationMode,
    pub params: VisualisationParams,
}
