use nexus_singularity::resonator::ResonatorCore;

fn main() {
    let temp = ResonatorCore::read_thermal_temp();
    println!("THERMAL_STATE:{:.2}", temp);
}