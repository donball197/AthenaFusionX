use crate::assessment::Assessment;

pub fn all(values: impl IntoIterator<Item = Assessment>) -> Assessment {
    values.into_iter().fold(Assessment::SC, Assessment::all)
}

pub fn any(values: impl IntoIterator<Item = Assessment>) -> Assessment {
    values.into_iter().fold(Assessment::NN, Assessment::any)
}
