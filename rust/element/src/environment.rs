/// The environment should hold the configration options that org uses.
pub trait Environment {}

pub struct DefaultEnvironment;

impl Environment for DefaultEnvironment {}
