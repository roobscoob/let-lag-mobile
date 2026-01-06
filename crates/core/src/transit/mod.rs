use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StationIdentifier(Arc<str>);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RouteIdentifier(Arc<str>);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ServiceIdentifier(Arc<str>);

pub struct StopEvent {
    pub location: StationIdentifier,
    pub arrival: u32, // seconds since service day start
    pub departure: u32,
}

pub trait Service {
    fn id(&self) -> ServiceIdentifier;

    /// Ordered stop events for a specific run
    fn stop_events(&self) -> &[StopEvent];
}

pub trait Route {
    fn id(&self) -> RouteIdentifier;
    fn services(&self) -> &[Arc<dyn Service>];
}

pub trait TransitStation {
    fn identifier(&self) -> StationIdentifier;
    fn name(&self) -> &str;
    fn complex(&self) -> Arc<dyn TransitComplex>;
}

pub trait TransitComplex {
    fn name(&self) -> &str;
    fn all_stations(&self) -> &[Arc<dyn TransitStation>];
    fn center(&self) -> geo::Point;
}

pub trait TransitProvider {
    fn all_complexes(&self) -> &[Arc<dyn TransitComplex>];
}
